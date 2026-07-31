use core::cell::Cell;
#[cfg(test)]
use core::ops::Range;

#[derive(Debug, PartialEq)]
pub struct Error;

#[derive(Debug, PartialEq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Span {
        Span { start, end }
    }

    pub fn splat(pos: Position) -> Span {
        Span::new(pos, pos)
    }
}

impl Position {
    pub fn new(offset: usize, line: usize, column: usize) -> Position {
        Position { offset, line, column }
    }
}

pub enum Ast {
    Empty(Box<Span>),
    Literal(Box<Literal>),
    Concat(Box<Concat>),
}

impl Ast {
    pub fn empty(span: Span) -> Ast {
        Ast::Empty(Box::new(span))
    }

    pub fn literal(e: Literal) -> Ast {
        Ast::Literal(Box::new(e))
    }

    pub fn concat(e: Concat) -> Ast {
        Ast::Concat(Box::new(e))
    }
}

pub struct Concat {
    pub span: Span,
    pub asts: Vec<Ast>,
}

impl Concat {
    pub fn into_ast(mut self) -> Ast {
        match self.asts.len() {
            0 => Ast::empty(self.span),
            1 => self.asts.pop().unwrap(),
            _ => Ast::concat(self),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Literal {
    pub span: Span,
    pub kind: LiteralKind,
    pub c: char,
}

#[derive(Debug, PartialEq)]
pub enum LiteralKind {
    Verbatim,
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
enum Primitive {
    Literal(Literal),
}

impl Primitive {
    fn into_ast(self) -> Ast {
        match self {
            Primitive::Literal(lit) => Ast::literal(lit),
        }
    }
}

pub struct ParserBuilder;

impl ParserBuilder {
    pub fn new() -> ParserBuilder {
        ParserBuilder
    }

    pub fn build(&self) -> Parser {
        Parser {
            pos: Cell::new(Position { offset: 0, line: 1, column: 1 }),
        }
    }
}

pub struct Parser {
    pos: Cell<Position>,
}

struct ParserI<'s> {
    parser: &'s mut Parser,
    pattern: &'s str,
}

impl Parser {
    pub fn new() -> Parser {
        ParserBuilder::new().build()
    }

    pub fn parse(&mut self, pattern: &str) -> Result<Ast> {
        ParserI::new(self, pattern).parse()
    }
}

impl<'s> ParserI<'s> {
    fn new(parser: &'s mut Parser, pattern: &'s str) -> ParserI<'s> {
        ParserI { parser, pattern }
    }

    fn parser(&self) -> &Parser {
        self.parser
    }

    fn pattern(&self) -> &str {
        self.pattern
    }

    fn offset(&self) -> usize {
        self.parser().pos.get().offset
    }

    fn line(&self) -> usize {
        self.parser().pos.get().line
    }

    fn column(&self) -> usize {
        self.parser().pos.get().column
    }

    fn char(&self) -> char {
        self.char_at(self.offset())
    }

    fn char_at(&self, i: usize) -> char {
        self.pattern()[i..]
            .chars()
            .next()
            .unwrap()
    }

    fn bump(&self) -> bool {
        if self.is_eof() {
            return false;
        }
        let Position { mut offset, mut line, mut column } = self.pos();
        if self.char() == '\n' {
            line = line.checked_add(1).unwrap();
            column = 1;
        } else {
            column = column.checked_add(1).unwrap();
        }
        offset += self.char().len_utf8();
        self.parser().pos.set(Position { offset, line, column });
        self.pattern()[self.offset()..].chars().next().is_some()
    }

    fn is_eof(&self) -> bool {
        self.offset() == self.pattern().len()
    }

    fn pos(&self) -> Position {
        self.parser().pos.get()
    }

    fn span(&self) -> Span {
        Span::splat(self.pos())
    }

    fn span_char(&self) -> Span {
        let mut next = Position {
            offset: self.offset().checked_add(self.char().len_utf8()).unwrap(),
            line: self.line(),
            column: self.column().checked_add(1).unwrap(),
        };
        if self.char() == '\n' {
            next.line += 1;
            next.column = 1;
        }
        Span::new(self.pos(), next)
    }
}

impl<'s> ParserI<'s> {
    fn parse(&self) -> Result<Ast> {
        let mut concat = Concat { span: self.span(), asts: vec![] };
        loop {
            if self.is_eof() {
                break;
            }
            match self.char() {
                _ => concat.asts.push(self.parse_primitive()?.into_ast()),
            }
        }
        Ok(concat.into_ast())
    }

    fn parse_primitive(&self) -> Result<Primitive> {
        match self.char() {
            c => {
                let ast = Primitive::Literal(Literal {
                    span: self.span_char(),
                    kind: LiteralKind::Verbatim,
                    c,
                });
                self.bump();
                Ok(ast)
            }
        }
    }
}

#[cfg(test)]
fn span(range: Range<usize>) -> Span {
    let start = Position::new(range.start, 1, range.start + 1);
    let end = Position::new(range.end, 1, range.end + 1);
    Span::new(start, end)
}

#[test]
fn parse_primitive_non_escape() {
    let mut p = Parser::new();
    assert_eq!(
        ParserI::new(&mut p, r"a").parse_primitive(),
        Ok(Primitive::Literal(Literal {
            span: span(0..1),
            kind: LiteralKind::Verbatim,
            c: 'a',
        }))
    );
}
