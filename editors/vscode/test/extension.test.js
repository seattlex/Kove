// Behaviour tests for the extension, run without VS Code.
//
// `vscode` and `child_process` are both replaced with stubs, so this
// exercises the real extension.js: which arguments reach the toolchain,
// what goes to it on stdin, and what ends up in the Problems panel. The
// bugs worth catching here are the ones about timing and bookkeeping,
// which reading the file does not reveal.
//
//     node editors/vscode/test/extension.test.js

const assert = require("assert");
const path = require("path");
const Module = require("module");

const extensionPath = path.join(__dirname, "..", "extension.js");

// --- Stubs ------------------------------------------------------------------

/** Calls to the toolchain, each resolvable by the test when it chooses. */
let calls;

function makeChildProcess() {
  return {
    execFile(file, args, options, callback) {
      const call = {
        file,
        args,
        options,
        stdin: undefined,
        finish(result = {}) {
          callback(
            result.error || null,
            result.stdout || "",
            result.stderr || ""
          );
        },
      };
      calls.push(call);
      return {
        stdin: {
          on() {},
          end(text) {
            call.stdin = text;
          },
        },
      };
    },
  };
}

function makeVscode(settings = {}) {
  const handlers = {};
  const on = (name) => (fn) => {
    (handlers[name] = handlers[name] || []).push(fn);
    return { dispose() {} };
  };

  const published = new Map();

  const vscode = {
    handlers,
    published,
    Position: class Position {
      constructor(line, character) {
        this.line = line;
        this.character = character;
      }
      isEqual(other) {
        return this.line === other.line && this.character === other.character;
      }
      translate(dl, dc) {
        return new vscode.Position(this.line + dl, this.character + dc);
      }
    },
    Range: class Range {
      constructor(start, end) {
        this.start = start;
        this.end = end;
      }
      contains() {
        return false;
      }
    },
    Diagnostic: class Diagnostic {
      constructor(range, message, severity) {
        this.range = range;
        this.message = message;
        this.severity = severity;
      }
    },
    DiagnosticSeverity: { Error: 0, Warning: 1 },
    TextEdit: {
      replace(range, newText) {
        return { range, newText };
      },
    },
    languages: {
      createDiagnosticCollection() {
        return {
          set(uri, list) {
            published.set(uri.toString(), list);
          },
          delete(uri) {
            published.delete(uri.toString());
          },
          dispose() {},
        };
      },
      registerDocumentFormattingEditProvider(_lang, provider) {
        vscode.formatter = provider;
        return { dispose() {} };
      },
      getDiagnostics() {
        return [];
      },
    },
    window: {
      activeTextEditor: undefined,
      createOutputChannel() {
        return {
          lines: [],
          appendLine(s) {
            this.lines.push(s);
          },
          append(s) {
            this.lines.push(s);
          },
          clear() {
            this.lines = [];
          },
          show() {},
          dispose() {},
        };
      },
      showErrorMessage() {
        return { then() {} };
      },
      showInformationMessage() {},
      showInputBox() {
        return Promise.resolve(undefined);
      },
    },
    commands: {
      registerCommand() {
        return { dispose() {} };
      },
      executeCommand() {},
    },
    workspace: {
      textDocuments: [],
      getConfiguration() {
        return {
          get(key, fallback) {
            return key in settings ? settings[key] : fallback;
          },
        };
      },
      getWorkspaceFolder() {
        return undefined;
      },
      onDidOpenTextDocument: on("open"),
      onDidSaveTextDocument: on("save"),
      onDidChangeTextDocument: on("change"),
      onDidCloseTextDocument: on("close"),
    },
  };
  return vscode;
}

function doc(fsPath, text) {
  return {
    languageId: "kove",
    uri: { fsPath, toString: () => `file://${fsPath}` },
    getText: () => text,
    positionAt(offset) {
      return { offset };
    },
    save: () => Promise.resolve(true),
  };
}

/**
 * Load a fresh copy of the extension against fresh stubs. The module
 * keeps state between activations, so every test gets its own.
 */
function activate(settings) {
  const vscode = makeVscode(settings);
  const childProcess = makeChildProcess();
  calls = [];

  const load = Module._load;
  Module._load = function (request, parent, isMain) {
    if (request === "vscode") return vscode;
    if (request === "child_process") return childProcess;
    return load.apply(this, [request, parent, isMain]);
  };
  delete require.cache[require.resolve(extensionPath)];
  const extension = require(extensionPath);
  Module._load = load;

  extension.activate({ subscriptions: [] });
  return { vscode, extension };
}

const fire = (vscode, name, arg) =>
  (vscode.handlers[name] || []).forEach((fn) => fn(arg));

/** Let debounce timers fire and promise callbacks run. */
const settle = (ms = 15) => new Promise((r) => setTimeout(r, ms));

const report = (diagnostics) => JSON.stringify({ file: "x.kov", diagnostics });

const oneError = (message) =>
  report([
    {
      severity: "error",
      code: "E0012",
      message,
      start: { line: 1, column: 1, offset: 0 },
      end: { line: 1, column: 5, offset: 4 },
    },
  ]);

// --- Tests ------------------------------------------------------------------

const tests = [];
const test = (name, fn) => tests.push({ name, fn });

test("the buffer is sent on stdin, not read from disk", async () => {
  const { vscode } = activate();
  fire(vscode, "open", doc("/w/a.kov", "fn main() { }"));
  await settle(0);

  assert.strictEqual(calls.length, 1);
  assert.deepStrictEqual(calls[0].args, [
    "check",
    "--json",
    "--name=/w/a.kov",
    "-",
  ]);
  assert.strictEqual(calls[0].stdin, "fn main() { }");
});

test("each document gets its own debounce timer", async () => {
  // The bug: one shared timer meant editing a second file cancelled the
  // first file's pending check, which then never ran.
  const { vscode } = activate({ checkDelay: 1 });
  fire(vscode, "change", { document: doc("/w/a.kov", "a") });
  fire(vscode, "change", { document: doc("/w/b.kov", "b") });
  await settle();

  const names = calls.map((c) => c.args.find((a) => a.startsWith("--name=")));
  assert.deepStrictEqual(names.sort(), ["--name=/w/a.kov", "--name=/w/b.kov"]);
});

test("repeated edits to one document collapse into a single check", async () => {
  const { vscode } = activate({ checkDelay: 5 });
  const d = doc("/w/a.kov", "a");
  fire(vscode, "change", { document: d });
  fire(vscode, "change", { document: d });
  fire(vscode, "change", { document: d });
  await settle(30);

  assert.strictEqual(calls.length, 1, "debouncing should leave one check");
});

test("closing a document cancels its pending check", async () => {
  const { vscode } = activate({ checkDelay: 5 });
  const d = doc("/w/a.kov", "a");
  fire(vscode, "change", { document: d });
  fire(vscode, "close", d);
  await settle(30);

  assert.strictEqual(calls.length, 0, "the check should not have run");
});

test("a slow check does not overwrite a newer one", async () => {
  // Checks are child processes and can finish out of order. The newest
  // one describes what is on screen, so it has to win regardless.
  const { vscode } = activate();
  const d = doc("/w/a.kov", "a");
  fire(vscode, "save", d);
  fire(vscode, "save", d);
  assert.strictEqual(calls.length, 2);

  calls[1].finish({ stdout: oneError("newer") });
  calls[0].finish({ stdout: oneError("older") });
  await settle(0);

  const shown = vscode.published.get("file:///w/a.kov");
  assert.strictEqual(shown.length, 1);
  assert.ok(
    shown[0].message.startsWith("newer"),
    `the later result should stand, got ${shown[0].message}`
  );
});

test("help and notes travel with the diagnostic", async () => {
  const { vscode } = activate();
  fire(vscode, "open", doc("/w/a.kov", "a"));
  calls[0].finish({
    stdout: report([
      {
        severity: "warning",
        code: "W0001",
        message: "`x` is never used",
        start: { line: 2, column: 5, offset: 10 },
        end: { line: 2, column: 6, offset: 11 },
        help: "remove it",
        notes: ["declared here"],
      },
    ]),
  });
  await settle(0);

  const [d] = vscode.published.get("file:///w/a.kov");
  assert.strictEqual(d.severity, vscode.DiagnosticSeverity.Warning);
  assert.strictEqual(d.code, "W0001");
  assert.ok(d.message.includes("help: remove it"));
  assert.ok(d.message.includes("note: declared here"));
  // Kove counts from 1, VS Code from 0.
  assert.strictEqual(d.range.start.line, 1);
  assert.strictEqual(d.range.start.character, 4);
});

test("a zero-width span still gets a visible squiggle", async () => {
  const { vscode } = activate();
  fire(vscode, "open", doc("/w/a.kov", "a"));
  calls[0].finish({
    stdout: report([
      {
        severity: "error",
        code: "E0101",
        message: "`;` expected here",
        start: { line: 1, column: 10, offset: 9 },
        end: { line: 1, column: 10, offset: 9 },
      },
    ]),
  });
  await settle(0);

  const [d] = vscode.published.get("file:///w/a.kov");
  assert.strictEqual(d.range.end.character, d.range.start.character + 1);
});

test("output that is not JSON leaves no diagnostics behind", async () => {
  const { vscode } = activate();
  fire(vscode, "open", doc("/w/a.kov", "a"));
  calls[0].finish({ stdout: oneError("real") });
  await settle(0);
  assert.ok(vscode.published.has("file:///w/a.kov"));

  fire(vscode, "save", doc("/w/a.kov", "a"));
  calls[1].finish({
    stderr: "error: `kove.toml` is not valid",
    error: { code: 2 },
  });
  await settle(0);
  assert.ok(
    !vscode.published.has("file:///w/a.kov"),
    "stale diagnostics should be cleared, not left on screen"
  );
});

test("formatting returns an edit and leaves the file alone", async () => {
  const { vscode } = activate();
  const d = doc("/w/a.kov", "fn main(){}");
  const edits = vscode.formatter.provideDocumentFormattingEdits(d);
  await settle(0);

  assert.deepStrictEqual(calls[0].args, ["fmt", "--name=/w/a.kov", "-"]);
  assert.strictEqual(calls[0].stdin, "fn main(){}");
  calls[0].finish({ stdout: "fn main() { }\n" });

  const [edit] = await edits;
  assert.strictEqual(edit.newText, "fn main() { }\n");
});

test("formatting unparseable source makes no edit", async () => {
  const { vscode } = activate();
  const edits = vscode.formatter.provideDocumentFormattingEdits(
    doc("/w/a.kov", "fn main() {")
  );
  await settle(0);
  calls[0].finish({ error: { code: 1 }, stderr: "error: ..." });

  assert.deepStrictEqual(await edits, []);
});

test("already formatted source makes no edit", async () => {
  const { vscode } = activate();
  const edits = vscode.formatter.provideDocumentFormattingEdits(
    doc("/w/a.kov", "fn main() { }\n")
  );
  await settle(0);
  calls[0].finish({ stdout: "fn main() { }\n" });

  assert.deepStrictEqual(await edits, [], "an empty edit is churn, not a format");
});

test("non-Kove documents are ignored", async () => {
  const { vscode } = activate({ checkDelay: 1 });
  const other = { ...doc("/w/a.rs", "fn main() {}"), languageId: "rust" };
  fire(vscode, "open", other);
  fire(vscode, "change", { document: other });
  await settle();

  assert.strictEqual(calls.length, 0);
});

test("checkOnType and checkOnSave are respected", async () => {
  const off = activate({ checkOnType: false, checkOnSave: false, checkDelay: 1 });
  fire(off.vscode, "change", { document: doc("/w/a.kov", "a") });
  fire(off.vscode, "save", doc("/w/a.kov", "a"));
  await settle();
  assert.strictEqual(calls.length, 0);
});

test("kove.path is what gets run", async () => {
  const { vscode } = activate({ path: "/opt/kove/bin/kove" });
  fire(vscode, "open", doc("/w/a.kov", "a"));
  await settle(0);
  assert.strictEqual(calls[0].file, "/opt/kove/bin/kove");
});

// --- Runner -----------------------------------------------------------------

(async () => {
  let failures = 0;
  for (const { name, fn } of tests) {
    try {
      await fn();
      console.log(`ok    ${name}`);
    } catch (e) {
      failures += 1;
      console.log(`FAIL  ${name}\n        ${e.message}`);
    }
  }
  console.log(
    failures === 0 ? "\nall checks passed" : `\n${failures} check(s) failed`
  );
  process.exit(failures === 0 ? 0 : 1);
})();
