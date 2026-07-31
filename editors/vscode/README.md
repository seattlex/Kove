# Kove for VS Code

Language support for [Kove](https://github.com/seattlex/Kove): syntax
highlighting, diagnostics, formatting, and the `kove` commands.

## What you get

- **Syntax highlighting** for `.kov` files, including compound assignment
  operators, `\u{...}` escapes and `Enum::Variant` paths.
- **Diagnostics** in the Problems panel, as you type and on save. The
  help and notes from a diagnostic appear in the hover, and the error
  code is attached so you can look it up.
- **Formatting** through `kove fmt`, so format-on-save works with no
  extra setup. It formats the buffer, saved or not. The formatter is
  opinionated and has no options, so neither does this.
- **Snippets** for the shapes you type constantly: `fn`, `main`, `test`,
  `struct`, `enum`, `for`, `if`.
- **Commands**, all in the palette:

  | Command | What it does |
  | --- | --- |
  | Kove: Run File | `kove run` on the current file, output in a panel |
  | Kove: Check File | Re-check now |
  | Kove: Run Tests | `kove test` on the current file |
  | Kove: Format File | Format now |
  | Kove: Explain Diagnostic Code | `kove explain`, offering the code under the cursor |
  | Kove: Show Toolchain Version | Which `kove` is being used |

## Requirements

The `kove` toolchain has to be installed and on your PATH:

```console
$ git clone https://github.com/seattlex/Kove
$ cd Kove && cargo build --release
$ export PATH="$PWD/target/release:$PATH"
```

If you would rather not touch PATH, set `kove.path` to the binary
instead. The extension says so plainly if it cannot run `kove`, with a
button that takes you to the setting.

## Settings

| Setting | Default | Meaning |
| --- | --- | --- |
| `kove.path` | `kove` | Path to the executable |
| `kove.checkOnSave` | `true` | Re-check when a file is saved |
| `kove.checkOnType` | `true` | Re-check after a pause in typing |
| `kove.checkDelay` | `400` | How long that pause is, in milliseconds |

## How it works

The extension is deliberately thin. It shells out to `kove` and shows
what comes back:

- diagnostics come from `kove check --json -`, a stable machine-readable
  format documented in [docs/diagnostics.md](../../docs/diagnostics.md)
- formatting is `kove fmt -`
- the commands run exactly what you would type in a terminal

Both read the buffer on stdin rather than the file on disk, so what you
see matches what you have typed rather than what you last saved.

So the editor cannot disagree with the compiler, and there is no second
implementation of the language to keep in step. When the Kove language
server lands, this extension will talk to it instead, and the behaviour
you see should not change.

## Installing from source

There is no build step. Either symlink the directory into your
extensions folder:

```console
$ ln -s "$PWD/editors/vscode" ~/.vscode/extensions/kove
```

or package it with [vsce](https://github.com/microsoft/vscode-vsce):

```console
$ cd editors/vscode && npx @vscode/vsce package
```

Reload the window afterwards.

## Known limits

- No completion, go-to-definition or rename yet. Those want the language
  server, which is v0.7 on the [roadmap](../../docs/roadmap.md). The
  compiler already computes what they need: the resolver maps every
  reference to what it names.
- Checking runs the whole frontend per keystroke pause. That is fast at
  the size of program Kove can express today; when it stops being fast,
  the answer is the language server and its incremental reparsing, not a
  cache here.
