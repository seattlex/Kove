//! The printer: walks the concrete syntax tree and emits canonical Kove.
//!
//! Line breaks are *pending* rather than written immediately. That is what
//! lets a trailing comment attach to the line it was written on: when the
//! comment turns up in the next token's leading trivia, the newline the
//! structure asked for has not been committed yet and can still be taken
//! back.

use crate::MAX_WIDTH;
use reparse::grammar::{Language, EOF_TOKEN};
use reparse::node::{SyntaxElem, SyntaxNode, SyntaxToken};

const INDENT: &str = "    ";

pub struct Printer<'a> {
    lang: &'a Language,
    text: &'a str,
    out: String,
    indent: usize,
    /// Line breaks owed to the output, not yet written.
    pending: usize,
    /// Set before a statement or item, so a gap in the source can become a
    /// blank line. Cleared once the gap has been considered, so blank lines
    /// never appear in the middle of an expression.
    at_boundary: bool,
}

impl<'a> Printer<'a> {
    pub fn new(lang: &'a Language, text: &'a str) -> Printer<'a> {
        Printer {
            lang,
            text,
            out: String::new(),
            indent: 0,
            pending: 0,
            at_boundary: false,
        }
    }

    pub fn finish(mut self) -> String {
        // Exactly one trailing newline, and never a file of blank lines.
        while self.out.ends_with(char::is_whitespace) {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out
    }

    // --- Output primitives ------------------------------------------------

    fn write(&mut self, s: &str) {
        self.flush();
        self.out.push_str(s);
    }

    fn flush(&mut self) {
        if self.pending == 0 {
            return;
        }
        // Nothing to separate from at the start of the file.
        if !self.out.is_empty() {
            for _ in 0..self.pending {
                self.out.push('\n');
            }
            for _ in 0..self.indent {
                self.out.push_str(INDENT);
            }
        }
        self.pending = 0;
    }

    fn newline(&mut self) {
        self.pending = self.pending.max(1);
    }

    fn blank_line(&mut self) {
        self.pending = self.pending.max(2);
    }

    /// Whether `width` more columns fit on the current line.
    ///
    /// The comparison is strict rather than `<=` on purpose: it holds one
    /// column back for a terminator the caller will add but this printer
    /// cannot see from here, such as the `;` after a statement or the `,`
    /// after a list item. Without that, a construct measuring as exactly
    /// full ends up one column over.
    fn fits(&self, width: usize) -> bool {
        self.current_column() + width < MAX_WIDTH
    }

    /// Width of the line being built, for the break-or-not decisions.
    fn current_column(&self) -> usize {
        if self.pending > 0 {
            return self.indent * INDENT.len();
        }
        match self.out.rfind('\n') {
            Some(i) => self.out.len() - i - 1,
            None => self.out.len(),
        }
    }

    // --- Tokens and comments ----------------------------------------------

    fn token_text(&self, tok: &SyntaxToken) -> &'a str {
        // The borrow is of the source text, not of `self`.
        let text: &'a str = self.text;
        let r = tok.text_range();
        &text[r.start as usize..r.end as usize]
    }

    /// Emit a token: its leading comments first, then the token itself.
    fn token(&mut self, tok: &SyntaxToken) {
        self.trivia(tok);
        if !tok.is_missing() {
            let text = self.token_text(tok);
            self.write(text);
        }
    }

    /// Emit a token preceded by a space (unless a line break is pending).
    fn spaced(&mut self, tok: &SyntaxToken) {
        self.trivia(tok);
        self.space();
        self.token_no_trivia(tok);
    }

    fn token_no_trivia(&mut self, tok: &SyntaxToken) {
        if !tok.is_missing() {
            let text = self.token_text(tok);
            self.write(text);
        }
    }

    fn space(&mut self) {
        if self.pending == 0 && !self.out.is_empty() && !self.out.ends_with(' ') {
            self.out.push(' ');
        }
    }

    /// Comments attached to a token, placed by where the author put them.
    ///
    /// A comment on the same line as the code before it stays there; a
    /// comment on its own line stays on its own line; a gap of one or more
    /// blank lines becomes exactly one blank line.
    fn trivia(&mut self, tok: &SyntaxToken) {
        let boundary = std::mem::take(&mut self.at_boundary);
        let mut newlines = 0usize;
        let mut first = true;

        for (piece, range) in tok.trivia().collect::<Vec<_>>() {
            let text: &'a str = self.text;
            let piece_text = &text[range.start as usize..range.end as usize];
            let name = self.lang.token_name(piece.kind);
            match name {
                "whitespace" => newlines += piece_text.matches('\n').count(),
                "line_comment" => {
                    if first && newlines == 0 && !self.out.is_empty() {
                        // Trails the code on the current line.
                        self.pending = 0;
                        self.space();
                        self.out.push_str(piece_text.trim_end());
                    } else {
                        if newlines >= 2 {
                            self.blank_line();
                        } else {
                            self.newline();
                        }
                        self.write(piece_text.trim_end());
                    }
                    // A line comment swallows the rest of its line.
                    self.pending = 1;
                    newlines = 0;
                    first = false;
                }
                "block_comment" => {
                    let inline = first && newlines == 0 && !self.out.is_empty();
                    if inline {
                        self.pending = 0;
                        self.space();
                        self.out.push_str(piece_text);
                        self.out.push(' ');
                    } else {
                        if newlines >= 2 {
                            self.blank_line();
                        } else {
                            self.newline();
                        }
                        self.write(piece_text);
                        self.pending = 1;
                    }
                    newlines = 0;
                    first = false;
                }
                _ => {}
            }
        }

        // A gap the author left between two statements or items becomes one
        // blank line. Inside an expression it is ignored.
        if boundary && newlines >= 2 {
            self.blank_line();
        }
    }

    /// True if any comment hides in this subtree, in which case it cannot
    /// be printed on a single line.
    fn has_comments(&self, node: &SyntaxNode) -> bool {
        node.descendants().any(|e| match e {
            SyntaxElem::Token(t) => t.trivia().any(|(piece, _)| {
                matches!(
                    self.lang.token_name(piece.kind),
                    "line_comment" | "block_comment"
                )
            }),
            SyntaxElem::Node(_) => false,
        })
    }

    // --- Tree helpers -----------------------------------------------------

    fn kind(&self, node: &SyntaxNode) -> &'a str {
        let lang: &'a Language = self.lang;
        lang.node_name(node.kind())
    }

    fn field(&self, node: &SyntaxNode, name: &str) -> Option<SyntaxElem> {
        node.child_by_field(self.lang.field(name)?)
    }

    fn field_node(&self, node: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
        match self.field(node, name)? {
            SyntaxElem::Node(n) => Some(n),
            _ => None,
        }
    }

    fn field_token(&self, node: &SyntaxNode, name: &str) -> Option<SyntaxToken> {
        match self.field(node, name)? {
            SyntaxElem::Token(t) => Some(t),
            _ => None,
        }
    }

    /// Tokens of this node that carry no field name, by their text. Used to
    /// reach keywords and punctuation so their comments are not lost.
    fn punct(&self, node: &SyntaxNode, text: &str) -> Option<SyntaxToken> {
        node.children().find_map(|e| match e {
            SyntaxElem::Token(t) if !t.is_missing() && self.token_text(&t) == text => Some(t),
            _ => None,
        })
    }

    fn children_of_kind(&self, node: &SyntaxNode, kind: &str) -> Vec<SyntaxNode> {
        node.child_nodes()
            .filter(|n| self.kind(n) == kind)
            .collect()
    }

    // --- Items ------------------------------------------------------------

    pub fn file(&mut self, root: &SyntaxNode) {
        let source = root
            .child_nodes()
            .find(|n| self.kind(n) == "source_file")
            .unwrap_or_else(|| root.clone());

        for (i, item) in source.child_nodes().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.at_boundary = true;
            self.item(&item);
        }

        // Comments after the last item hang off the end-of-file token.
        for elem in root.children() {
            if let SyntaxElem::Token(t) = elem {
                if t.kind() == EOF_TOKEN {
                    self.at_boundary = true;
                    self.trivia(&t);
                }
            }
        }
    }

    fn item(&mut self, node: &SyntaxNode) {
        match self.kind(node) {
            "function_declaration" => self.function(node),
            "struct_declaration" => self.struct_decl(node),
            "enum_declaration" => self.enum_decl(node),
            "import_declaration" => self.import_decl(node),
            // A statement at the top level does not compile, but the
            // formatter is not the stage that complains about it.
            _ => self.stmt(node),
        }
    }

    fn function(&mut self, node: &SyntaxNode) {
        if let Some(kw) = self.punct(node, "fn") {
            self.token(&kw);
        }
        if let Some(name) = self.field_token(node, "name") {
            self.trivia(&name);
            self.space();
            self.token_no_trivia(&name);
        }
        if let Some(params) = self.field_node(node, "params") {
            self.parameter_list(&params);
        }
        if let Some(ret) = self.field_node(node, "return_type") {
            self.write(" -> ");
            self.type_expr(&ret);
        }
        self.write(" ");
        if let Some(body) = self.field_node(node, "body") {
            self.block(&body);
        }
        self.newline();
    }

    fn parameter_list(&mut self, node: &SyntaxNode) {
        let params = self.children_of_kind(node, "parameter");
        if params.is_empty() {
            self.write("()");
            return;
        }
        if !self.has_comments(node) {
            if let Some(inline) = self.render_list(&params, Printer::parameter) {
                // Plus the two parentheses.
                if self.fits(inline.len() + 2) {
                    self.write("(");
                    self.write(&inline);
                    self.write(")");
                    return;
                }
            }
        }
        // Too wide: one parameter per line, with a trailing comma so
        // adding another touches one line.
        self.write("(");
        self.indent += 1;
        for p in params.iter() {
            self.newline();
            self.parameter(p);
            self.write(",");
        }
        self.indent -= 1;
        self.newline();
        self.write(")");
    }

    fn parameter(&mut self, node: &SyntaxNode) {
        if let Some(name) = self.field_token(node, "name") {
            self.token(&name);
        }
        self.write(": ");
        if let Some(ty) = self.field_node(node, "type") {
            self.type_expr(&ty);
        }
    }

    fn type_expr(&mut self, node: &SyntaxNode) {
        if let Some(name) = self.field_token(node, "name") {
            self.token(&name);
        }
    }

    fn struct_decl(&mut self, node: &SyntaxNode) {
        if let Some(kw) = self.punct(node, "struct") {
            self.token(&kw);
        }
        if let Some(name) = self.field_token(node, "name") {
            self.spaced(&name);
        }
        let fields = self.children_of_kind(node, "field_declaration");
        self.braced_members(node, &fields, |p, f| {
            if let Some(name) = p.field_token(f, "name") {
                p.token(&name);
            }
            p.write(": ");
            if let Some(ty) = p.field_node(f, "type") {
                p.type_expr(&ty);
            }
        });
        self.newline();
    }

    fn enum_decl(&mut self, node: &SyntaxNode) {
        if let Some(kw) = self.punct(node, "enum") {
            self.token(&kw);
        }
        if let Some(name) = self.field_token(node, "name") {
            self.spaced(&name);
        }
        let variants = self.children_of_kind(node, "variant");
        self.braced_members(node, &variants, |p, v| {
            if let Some(name) = p.field_token(v, "name") {
                p.token(&name);
            }
        });
        self.newline();
    }

    /// Declarations always put one member per line: they are read far more
    /// often than they are written, and a diff that touches one field
    /// should touch one line.
    fn braced_members(
        &mut self,
        node: &SyntaxNode,
        members: &[SyntaxNode],
        mut print: impl FnMut(&mut Printer<'a>, &SyntaxNode),
    ) {
        self.write(" {");
        if members.is_empty() {
            // Preserve a comment sitting inside otherwise empty braces.
            if let Some(close) = self.punct(node, "}") {
                self.indent += 1;
                self.trivia(&close);
                self.indent -= 1;
                self.newline_if_wrote_comment();
                self.write("}");
            } else {
                self.write("}");
            }
            return;
        }
        self.indent += 1;
        for (i, m) in members.iter().enumerate() {
            if i > 0 {
                self.write(",");
            }
            self.newline();
            self.at_boundary = true;
            print(self, m);
        }
        self.indent -= 1;
        self.newline();
        if let Some(close) = self.punct(node, "}") {
            self.trivia(&close);
        }
        self.write("}");
    }

    /// After emitting comments inside empty braces, the closing brace needs
    /// its own line.
    fn newline_if_wrote_comment(&mut self) {
        if self.pending > 0 {
            self.pending = 1;
        }
    }

    fn import_decl(&mut self, node: &SyntaxNode) {
        if let Some(kw) = self.punct(node, "import") {
            self.token(&kw);
        }
        if let Some(path) = self.field_node(node, "path") {
            self.space();
            let mut first = true;
            for elem in path.children() {
                if let SyntaxElem::Token(t) = elem {
                    if t.is_missing() {
                        continue;
                    }
                    if first {
                        self.token_no_trivia(&t);
                        first = false;
                    } else {
                        let text = self.token_text(&t);
                        self.write(text);
                    }
                }
            }
        }
        self.write(";");
        self.newline();
    }

    // --- Statements -------------------------------------------------------

    fn block(&mut self, node: &SyntaxNode) {
        let stmts: Vec<SyntaxNode> = node.child_nodes().collect();
        self.write("{");
        if stmts.is_empty() {
            if let Some(close) = self.punct(node, "}") {
                self.indent += 1;
                self.trivia(&close);
                self.indent -= 1;
                self.newline_if_wrote_comment();
            }
            self.write("}");
            return;
        }
        self.indent += 1;
        for stmt in &stmts {
            self.newline();
            self.at_boundary = true;
            self.stmt(stmt);
        }
        self.indent -= 1;
        self.newline();
        if let Some(close) = self.punct(node, "}") {
            self.trivia(&close);
        }
        self.write("}");
    }

    fn stmt(&mut self, node: &SyntaxNode) {
        match self.kind(node) {
            "let_statement" => {
                if let Some(kw) = self.punct(node, "let") {
                    self.token(&kw);
                }
                if let Some(m) = self.field_token(node, "mutable") {
                    self.spaced(&m);
                }
                if let Some(name) = self.field_token(node, "name") {
                    self.spaced(&name);
                }
                if let Some(ty) = self.field_node(node, "type") {
                    self.write(": ");
                    self.type_expr(&ty);
                }
                self.write(" = ");
                if let Some(value) = self.field(node, "value") {
                    self.expr(&value);
                }
                self.write(";");
            }
            "assignment_statement" => {
                if let Some(target) = self.field(node, "target") {
                    self.expr(&target);
                }
                self.write(" = ");
                if let Some(value) = self.field(node, "value") {
                    self.expr(&value);
                }
                self.write(";");
            }
            "return_statement" => {
                if let Some(kw) = self.punct(node, "return") {
                    self.token(&kw);
                }
                if let Some(value) = self.field(node, "value") {
                    self.space();
                    self.expr(&value);
                }
                self.write(";");
            }
            "expression_statement" => {
                if let Some(value) = self.field(node, "value") {
                    self.expr(&value);
                }
                self.write(";");
            }
            "if_statement" => self.if_stmt(node),
            "while_statement" => {
                if let Some(kw) = self.punct(node, "while") {
                    self.token(&kw);
                }
                self.space();
                if let Some(cond) = self.field(node, "condition") {
                    self.expr(&cond);
                }
                self.write(" ");
                if let Some(body) = self.field_node(node, "body") {
                    self.block(&body);
                }
            }
            "for_statement" => {
                if let Some(kw) = self.punct(node, "for") {
                    self.token(&kw);
                }
                if let Some(name) = self.field_token(node, "name") {
                    self.spaced(&name);
                }
                self.write(" in ");
                if let Some(iter) = self.field(node, "iter") {
                    self.expr(&iter);
                }
                self.write(" ");
                if let Some(body) = self.field_node(node, "body") {
                    self.block(&body);
                }
            }
            "block" => self.block(node),
            // An error island cannot occur here: `format` refuses input
            // that does not parse cleanly.
            _ => {}
        }
    }

    fn if_stmt(&mut self, node: &SyntaxNode) {
        if let Some(kw) = self.punct(node, "if") {
            self.token(&kw);
        }
        self.space();
        if let Some(cond) = self.field(node, "condition") {
            self.expr(&cond);
        }
        self.write(" ");
        if let Some(then) = self.field_node(node, "then") {
            self.block(&then);
        }
        if let Some(else_branch) = self.field_node(node, "else") {
            self.write(" else ");
            if self.kind(&else_branch) == "if_statement" {
                self.if_stmt(&else_branch);
            } else {
                self.block(&else_branch);
            }
        }
    }

    // --- Expressions ------------------------------------------------------

    fn expr(&mut self, elem: &SyntaxElem) {
        match elem {
            SyntaxElem::Token(t) => self.token(t),
            SyntaxElem::Node(n) => self.expr_node(n),
        }
    }

    fn expr_node(&mut self, node: &SyntaxNode) {
        match self.kind(node) {
            "binary_expression" => {
                if let Some(lhs) = self.field(node, "lhs") {
                    self.expr(&lhs);
                }
                if let Some(op) = self.field_token(node, "op") {
                    self.trivia(&op);
                    self.space();
                    self.token_no_trivia(&op);
                }
                self.write(" ");
                if let Some(rhs) = self.field(node, "rhs") {
                    self.expr(&rhs);
                }
            }
            // `0..10` reads as one thing; spaces would break it up.
            "range_expression" => {
                if let Some(lhs) = self.field(node, "lhs") {
                    self.expr(&lhs);
                }
                self.write("..");
                if let Some(rhs) = self.field(node, "rhs") {
                    self.expr(&rhs);
                }
            }
            "unary_expression" => {
                if let Some(op) = self.field_token(node, "op") {
                    self.token(&op);
                }
                if let Some(operand) = self.field(node, "operand") {
                    self.expr(&operand);
                }
            }
            "call_expression" => {
                if let Some(callee) = self.field(node, "callee") {
                    self.expr(&callee);
                }
                for args in node.child_nodes() {
                    if self.kind(&args) == "arguments" {
                        self.arguments(&args);
                    }
                }
            }
            "field_expression" => {
                if let Some(base) = self.field(node, "base") {
                    self.expr(&base);
                }
                self.write(".");
                if let Some(name) = self.field_token(node, "name") {
                    self.token_no_trivia(&name);
                }
            }
            "paren_expression" => {
                self.write("(");
                if let Some(inner) = self.field(node, "inner") {
                    self.expr(&inner);
                }
                self.write(")");
            }
            "path_expression" => {
                if let Some(base) = self.field_token(node, "base") {
                    self.token(&base);
                }
                self.write("::");
                if let Some(name) = self.field_token(node, "name") {
                    self.token_no_trivia(&name);
                }
            }
            "struct_literal" => self.struct_literal(node),
            _ => {}
        }
    }

    fn arguments(&mut self, node: &SyntaxNode) {
        let args: Vec<SyntaxElem> = match self.lang.field("arg") {
            Some(f) => node.children().filter(|c| c.field() == Some(f)).collect(),
            None => Vec::new(),
        };
        if args.is_empty() {
            self.write("()");
            return;
        }
        if !self.has_comments(node) {
            if let Some(inline) = self.render_list(&args, Printer::expr) {
                if self.fits(inline.len() + 2) {
                    self.write("(");
                    self.write(&inline);
                    self.write(")");
                    return;
                }
            }
        }
        self.write("(");
        self.indent += 1;
        for a in args.iter() {
            self.newline();
            self.expr(a);
            self.write(",");
        }
        self.indent -= 1;
        self.newline();
        self.write(")");
    }

    /// A struct literal stays on one line when it fits and has no comments
    /// in it, and breaks one field per line when it does not. The decision
    /// is made from the printed width, so it depends on the code and not on
    /// how the author happened to type it.
    fn struct_literal(&mut self, node: &SyntaxNode) {
        let inits = self.children_of_kind(node, "field_initializer");
        if let Some(name) = self.field_token(node, "name") {
            self.token(&name);
        }
        if inits.is_empty() {
            self.write(" {}");
            return;
        }

        if !self.has_comments(node) {
            if let Some(inline) = self.render_inline(&inits) {
                // Plus the space before the opening brace.
                if self.fits(inline.len() + 1) {
                    self.write(" ");
                    self.write(&inline);
                    return;
                }
            }
        }

        self.write(" {");
        self.indent += 1;
        for (i, init) in inits.iter().enumerate() {
            if i > 0 {
                self.write(",");
            }
            self.newline();
            self.at_boundary = true;
            self.field_initializer(init);
        }
        self.indent -= 1;
        self.newline();
        if let Some(close) = self.punct(node, "}") {
            self.trivia(&close);
        }
        self.write("}");
    }

    fn field_initializer(&mut self, node: &SyntaxNode) {
        if let Some(name) = self.field_token(node, "name") {
            self.token(&name);
        }
        self.write(": ");
        if let Some(value) = self.field(node, "value") {
            self.expr(&value);
        }
    }

    /// Render field initializers as `{ a: 1, b: 2 }` on a scratch printer,
    /// to measure whether they fit.
    fn render_inline(&self, inits: &[SyntaxNode]) -> Option<String> {
        let mut scratch = Printer::new(self.lang, self.text);
        scratch.out.push_str("{ ");
        for (i, init) in inits.iter().enumerate() {
            if i > 0 {
                scratch.write(", ");
            }
            scratch.field_initializer(init);
        }
        scratch.write(" }");
        if scratch.out.contains('\n') {
            return None;
        }
        Some(scratch.out)
    }

    /// Render a comma-separated list on a scratch printer, to measure
    /// whether it fits on the current line. `None` if it cannot be one
    /// line at all.
    fn render_list<T>(
        &self,
        items: &[T],
        mut print: impl FnMut(&mut Printer<'a>, &T),
    ) -> Option<String> {
        let mut scratch = Printer::new(self.lang, self.text);
        // A non-empty scratch buffer keeps `space()` and the comment
        // placement rules behaving as they would mid-line.
        scratch.out.push('(');
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                scratch.write(", ");
            }
            print(&mut scratch, item);
        }
        if scratch.out.contains('\n') {
            return None;
        }
        Some(scratch.out[1..].to_string())
    }
}
