// Kove language support for VS Code.
//
// The extension is deliberately thin: everything it knows comes from the
// `kove` binary. Diagnostics are `kove check --json`, formatting is
// `kove fmt`, and the commands run the same thing you would type. That
// means the editor can never disagree with the compiler, and there is no
// second implementation of anything to keep in step.
//
// Plain JavaScript on purpose. No build step, no bundler, no transpile:
// what is in the repository is what runs.

const vscode = require("vscode");
const { execFile } = require("child_process");
const path = require("path");

/** Diagnostics shown in the Problems panel, keyed by file. */
let diagnostics;
/** Pending debounce timer for check-on-type. */
let pending;
/** Output channel for anything the user should be able to read afterwards. */
let output;

function activate(context) {
  diagnostics = vscode.languages.createDiagnosticCollection("kove");
  output = vscode.window.createOutputChannel("Kove");
  context.subscriptions.push(diagnostics, output);

  context.subscriptions.push(
    vscode.commands.registerCommand("kove.run", () => runCommand("run")),
    vscode.commands.registerCommand("kove.check", () => checkActiveDocument()),
    vscode.commands.registerCommand("kove.test", () => runCommand("test")),
    vscode.commands.registerCommand("kove.format", () =>
      vscode.commands.executeCommand("editor.action.formatDocument")
    ),
    vscode.commands.registerCommand("kove.explain", explainCommand),
    vscode.commands.registerCommand("kove.showVersion", showVersion)
  );

  // Formatting goes through `kove fmt`, so format-on-save works with no
  // extra configuration.
  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider("kove", {
      provideDocumentFormattingEdits: formatDocument,
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument(checkIfKove),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (config().get("checkOnSave", true)) checkIfKove(doc);
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (!config().get("checkOnType", true)) return;
      if (event.document.languageId !== "kove") return;
      clearTimeout(pending);
      pending = setTimeout(
        () => check(event.document),
        config().get("checkDelay", 400)
      );
    }),
    vscode.workspace.onDidCloseTextDocument((doc) => diagnostics.delete(doc.uri))
  );

  vscode.workspace.textDocuments.forEach(checkIfKove);
}

function deactivate() {
  clearTimeout(pending);
}

function config() {
  return vscode.workspace.getConfiguration("kove");
}

function koveExecutable() {
  return config().get("path", "kove") || "kove";
}

function checkIfKove(doc) {
  if (doc && doc.languageId === "kove") check(doc);
}

function checkActiveDocument() {
  const editor = vscode.window.activeTextEditor;
  if (editor && editor.document.languageId === "kove") check(editor.document);
}

/**
 * Run the toolchain and hand back stdout, stderr and the exit code.
 *
 * A non-zero exit is not an error here: `kove check` exits 1 when the
 * program has errors, which is exactly the case we want to read.
 *
 * `options.stdin` feeds the process source text, which is how the
 * editor gets the buffer to the compiler rather than whatever was last
 * written to disk.
 */
function runKove(args, options = {}) {
  return new Promise((resolve) => {
    const child = execFile(
      koveExecutable(),
      args,
      { cwd: options.cwd, maxBuffer: 16 * 1024 * 1024 },
      (error, stdout, stderr) => {
        const spawnFailed = error && error.code === "ENOENT";
        resolve({
          stdout: stdout || "",
          stderr: stderr || "",
          code: error && typeof error.code === "number" ? error.code : 0,
          spawnFailed,
        });
      }
    );
    if (options.stdin !== undefined && child.stdin) {
      // The process may reject its arguments and exit before reading, so
      // a broken pipe here is not something to report.
      child.stdin.on("error", () => {});
      child.stdin.end(options.stdin);
    }
  });
}

let missingToolchainReported = false;

function reportMissingToolchain() {
  if (missingToolchainReported) return;
  missingToolchainReported = true;
  vscode.window
    .showErrorMessage(
      `Could not run \`${koveExecutable()}\`. Install the Kove toolchain, or set "kove.path" to its location.`,
      "Open Settings"
    )
    .then((choice) => {
      if (choice === "Open Settings") {
        vscode.commands.executeCommand(
          "workbench.action.openSettings",
          "kove.path"
        );
      }
    });
}

/** Directory to run in, so project-relative behaviour matches the terminal. */
function workingDirectory(doc) {
  const folder = vscode.workspace.getWorkspaceFolder(doc.uri);
  if (folder) return folder.uri.fsPath;
  return path.dirname(doc.uri.fsPath);
}

async function check(doc) {
  // The buffer goes to the compiler on stdin, so what you see matches
  // what you have typed rather than what was last saved. `--name` keeps
  // the real path on the diagnostics.
  const result = await runKove(
    ["check", "--json", `--name=${doc.uri.fsPath}`, "-"],
    { cwd: workingDirectory(doc), stdin: doc.getText() }
  );
  if (result.spawnFailed) {
    reportMissingToolchain();
    return;
  }

  let report;
  try {
    report = JSON.parse(result.stdout);
  } catch (e) {
    // Exit code 2 means the CLI refused before producing JSON, such as a
    // manifest that does not parse. Say so rather than failing silently.
    if (result.stderr.trim()) {
      output.appendLine(result.stderr.trim());
    }
    diagnostics.delete(doc.uri);
    return;
  }

  diagnostics.set(doc.uri, (report.diagnostics || []).map(toVscodeDiagnostic));
}

function toVscodeDiagnostic(d) {
  // Kove counts lines and columns from 1; VS Code counts from 0.
  const start = new vscode.Position(d.start.line - 1, d.start.column - 1);
  const end = new vscode.Position(d.end.line - 1, d.end.column - 1);
  // A zero-width span (a missing token) would be invisible, so widen it
  // by one character to give the squiggle something to sit under.
  const range = start.isEqual(end)
    ? new vscode.Range(start, start.translate(0, 1))
    : new vscode.Range(start, end);

  const severity =
    d.severity === "warning"
      ? vscode.DiagnosticSeverity.Warning
      : vscode.DiagnosticSeverity.Error;

  // The label belongs on the squiggle; help and notes are the sort of
  // thing you want when you hover, so they go on the message.
  let message = d.message;
  if (d.help) message += `\nhelp: ${d.help}`;
  for (const note of d.notes || []) message += `\nnote: ${note}`;

  const diagnostic = new vscode.Diagnostic(range, message, severity);
  diagnostic.source = "kove";
  diagnostic.code = d.code;
  return diagnostic;
}

async function formatDocument(doc) {
  // Format the buffer through stdin and return edits, which is the
  // contract VS Code expects: the editor applies them, nothing writes to
  // disk behind its back, and an unsaved file formats like any other.
  const before = doc.getText();
  const result = await runKove(["fmt", `--name=${doc.uri.fsPath}`, "-"], {
    cwd: workingDirectory(doc),
    stdin: before,
  });
  if (result.spawnFailed) {
    reportMissingToolchain();
    return [];
  }
  if (result.code !== 0) {
    // It does not parse; the diagnostics already say why, so there is
    // nothing useful to add here.
    return [];
  }

  const formatted = result.stdout;
  if (formatted === before) return [];

  const whole = new vscode.Range(
    doc.positionAt(0),
    doc.positionAt(before.length)
  );
  return [vscode.TextEdit.replace(whole, formatted)];
}

async function runCommand(subcommand) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "kove") return;
  await editor.document.save();

  const doc = editor.document;
  output.clear();
  output.show(true);
  output.appendLine(`$ kove ${subcommand} ${path.basename(doc.uri.fsPath)}`);

  const result = await runKove([subcommand, doc.uri.fsPath], {
    cwd: workingDirectory(doc),
  });
  if (result.spawnFailed) {
    reportMissingToolchain();
    return;
  }
  if (result.stdout) output.append(result.stdout);
  if (result.stderr) output.append(result.stderr);
  output.appendLine(`\n[exit ${result.code}]`);
  check(doc);
}

async function explainCommand() {
  // Offer the code under the cursor, if the cursor is on a diagnostic.
  const editor = vscode.window.activeTextEditor;
  let suggestion = "";
  if (editor) {
    const here = vscode.languages
      .getDiagnostics(editor.document.uri)
      .find((d) => d.range.contains(editor.selection.active));
    if (here && here.code) suggestion = String(here.code);
  }

  const code = await vscode.window.showInputBox({
    prompt: "Diagnostic code to explain",
    placeHolder: "E0012",
    value: suggestion,
  });
  if (!code) return;

  const result = await runKove(["explain", code]);
  if (result.spawnFailed) {
    reportMissingToolchain();
    return;
  }
  output.clear();
  output.show(true);
  output.append(result.stdout || result.stderr);
}

async function showVersion() {
  const result = await runKove(["version"]);
  if (result.spawnFailed) {
    reportMissingToolchain();
    return;
  }
  vscode.window.showInformationMessage(
    result.stdout.trim() || result.stderr.trim()
  );
}

module.exports = { activate, deactivate };
