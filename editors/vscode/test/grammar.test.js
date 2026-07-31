// Checks on the extension's static files.
//
// This is not a full TextMate engine, so it cannot assert exactly which
// scope a given character ends up with. What it can do is catch the
// mistakes that actually happen: a JSON file that stops parsing, a regex
// that no longer compiles, a pattern that has drifted away from the
// language, and a command declared in one place but not the other.
//
//     node editors/vscode/test/grammar.test.js

const assert = require("assert");
const fs = require("fs");
const path = require("path");

const here = __dirname;
const root = path.join(here, "..");
const load = (p) => JSON.parse(fs.readFileSync(path.join(root, p), "utf8"));

const pkg = load("package.json");
const grammar = load("syntaxes/kove.tmLanguage.json");
const langConfig = load("language-configuration.json");
const snippets = load("snippets/kove.json");

let failures = 0;
function test(name, fn) {
  try {
    fn();
    console.log(`ok    ${name}`);
  } catch (e) {
    failures += 1;
    console.log(`FAIL  ${name}\n        ${e.message}`);
  }
}

/** Every match/begin/end regex in the grammar, with where it came from. */
function patterns(node, where = "root", out = []) {
  if (Array.isArray(node)) {
    node.forEach((v, i) => patterns(v, `${where}[${i}]`, out));
  } else if (node && typeof node === "object") {
    for (const [k, v] of Object.entries(node)) {
      if (["match", "begin", "end"].includes(k) && typeof v === "string") {
        out.push({ where: `${where}.${k}`, source: v });
      } else {
        patterns(v, `${where}.${k}`, out);
      }
    }
  }
  return out;
}

test("every grammar regex compiles", () => {
  for (const { where, source } of patterns(grammar)) {
    try {
      new RegExp(source);
    } catch (e) {
      throw new Error(`${where}: ${source}\n        ${e.message}`);
    }
  }
});

test("the grammar covers every keyword the language reserves", () => {
  // Kept in step with docs/language.md by hand; this catches the case
  // where a keyword is added to the language and not to the highlighting.
  const reserved = [
    "fn", "let", "mut", "return", "if", "else", "while", "for", "in",
    "struct", "enum", "import", "true", "false", "match",
  ];
  const text = JSON.stringify(grammar);
  for (const word of reserved) {
    assert.ok(
      text.includes(word),
      `keyword \`${word}\` does not appear in the grammar`
    );
  }
});

test("the grammar's built-ins are the compiler's built-ins", () => {
  // The one list in this file that can be checked against the source of
  // truth instead of maintained by hand. Adding a builtin to the
  // resolver and not here would leave it highlighted as user code.
  const resolver = fs.readFileSync(
    path.join(root, "../../compiler/resolver/src/lib.rs"),
    "utf8"
  );
  const impl = resolver.slice(resolver.indexOf("impl Builtin"));
  const expected = [...impl.matchAll(/Builtin::\w+ => "(\w+)"/g)].map(
    (m) => m[1]
  );
  assert.ok(expected.length > 0, "found no builtin names in the resolver");

  const builtins = grammar.repository.calls.patterns.find((p) =>
    (p.name || "").startsWith("support.function.builtin")
  );
  assert.ok(builtins, "no built-in call pattern in the grammar");
  const listed = /\(([a-z_|]+)\)/.exec(builtins.match);
  assert.ok(listed, `no alternation in ${builtins.match}`);
  assert.deepStrictEqual(
    listed[1].split("|").sort(),
    expected.sort(),
    "the grammar and the resolver disagree about what is built in"
  );
});

test("keyword patterns use word boundaries", () => {
  // Without \b, `information` would highlight `in` and `format` would
  // highlight `for`.
  for (const { where, source } of patterns(grammar)) {
    if (source.includes("if|else") || source.includes("fn|let")) {
      assert.ok(
        source.startsWith("\\b") && source.endsWith("\\b"),
        `${where} is missing word boundaries: ${source}`
      );
    }
  }
});

test("float is matched before int", () => {
  const literals = JSON.stringify(grammar.repository.literals.patterns);
  const float = literals.indexOf("numeric.float");
  const int = literals.indexOf("numeric.integer");
  assert.ok(float !== -1 && int !== -1, "both literal patterns exist");
  assert.ok(float < int, "1.5 would otherwise be read as 1 then .5");
});

test("compound assignment is matched before the single-character forms", () => {
  const ops = JSON.stringify(grammar.repository.operators.patterns);
  const compound = ops.indexOf("assignment.compound");
  const plain = ops.indexOf("keyword.operator.assignment.kove");
  assert.ok(compound !== -1 && plain !== -1);
  assert.ok(compound < plain, "`+=` would otherwise be read as `+` then `=`");
});

test("the language is registered for .kov", () => {
  const language = pkg.contributes.languages[0];
  assert.strictEqual(language.id, "kove");
  assert.deepStrictEqual(language.extensions, [".kov"]);
  assert.strictEqual(
    pkg.contributes.grammars[0].scopeName,
    grammar.scopeName,
    "the grammar's scope name has to match what package.json declares"
  );
});

test("every declared command is implemented", () => {
  const source = fs.readFileSync(path.join(root, "extension.js"), "utf8");
  for (const { command } of pkg.contributes.commands) {
    assert.ok(
      source.includes(`"${command}"`),
      `${command} is declared but never registered`
    );
  }
});

test("every registered command is declared", () => {
  const source = fs.readFileSync(path.join(root, "extension.js"), "utf8");
  const registered = [...source.matchAll(/registerCommand\("([^"]+)"/g)].map(
    (m) => m[1]
  );
  const declared = pkg.contributes.commands.map((c) => c.command);
  for (const command of registered) {
    assert.ok(
      declared.includes(command),
      `${command} is registered but not in package.json, so it will not appear in the palette`
    );
  }
});

test("every setting the code reads is declared", () => {
  const source = fs.readFileSync(path.join(root, "extension.js"), "utf8");
  const read = [...source.matchAll(/config\(\)\.get\("([^"]+)"/g)].map(
    (m) => `kove.${m[1]}`
  );
  const declared = Object.keys(pkg.contributes.configuration.properties);
  for (const setting of read) {
    assert.ok(
      declared.includes(setting),
      `${setting} is read but not declared, so it has no default`
    );
  }
});

test("comments match the language", () => {
  assert.strictEqual(langConfig.comments.lineComment, "//");
  assert.deepStrictEqual(langConfig.comments.blockComment, ["/*", "*/"]);
});

test("snippets are well formed", () => {
  for (const [name, snippet] of Object.entries(snippets)) {
    assert.ok(snippet.prefix, `${name} has no prefix`);
    assert.ok(Array.isArray(snippet.body), `${name} body should be lines`);
    assert.ok(snippet.description, `${name} has no description`);
  }
});

console.log(
  failures === 0
    ? "\nall checks passed"
    : `\n${failures} check(s) failed`
);
process.exit(failures === 0 ? 0 : 1);
