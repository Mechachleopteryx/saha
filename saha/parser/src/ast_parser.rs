//! AST parser
//!
//! Parses AST from function/method bodies.

use std::{
    iter::{Iterator, Peekable},
    slice::Iter,
    mem::discriminant,
};

use saha_lib::prelude::*;

use saha_lib::{
    ast::*,
    source::token::Token
};

use crate::{
    parser::{
        TokenType,
        PR,
        ParsesTokens
    }
};

/// AstParser, which parses functions and methods from tokens into ASTs.
pub struct AstParser<'a> {
    ctok: Option<&'a Token>,
    ptok: Option<&'a Token>,
    ntok: Option<&'a Token>,
    tokidx: usize,
    shadow: &'a [Token],
    tokens: Peekable<Iter<'a, Token>>
}

impl<'a> ParsesTokens for AstParser<'a> {
    fn consume_next(&mut self, next_variants: Vec<&str>) -> PR<()> {
        let next_discriminants: Vec<TokenType> = next_variants.clone().iter().map(|a| -> TokenType {
            self.get_dummy_token_type(a)
        }).collect();

        self.ptok = self.ctok;
        self.ctok = self.tokens.next();

        if self.ctok.is_none() {
            if self.ptok.is_some() {
                return Err(ParseError::new(
                    &format!("Unexpected end of token stream after `{}` token", self.ptok.unwrap()),
                    Some(self.ptok.unwrap().get_file_position())
                ));
            }

            return Err(ParseError::new("Unexpected end of token stream", Some(FilePosition::unknown())));
        }

        if !next_discriminants.contains(&discriminant(&self.ctok.unwrap().clone())) {
            let unexp = self.ctok.unwrap().clone();
            return self.unexpected(&unexp, next_variants);
        }

        let next = {
            self.tokens.peek()
        };

        if next.is_none() {
            self.ntok = None;
        } else {
            self.ntok = next.cloned();
        }

        self.advance_token_index();

        return Ok(());
    }

    fn consume_any(&mut self) -> PR<()> {
        self.ptok = self.ctok;
        self.ctok = self.tokens.next();

        if self.ctok.is_none() {
            if self.ptok.is_some() {
                return Err(ParseError::new(
                    &format!("Unexpected end of token stream after `{}` token", self.ptok.unwrap()),
                    Some(self.ptok.unwrap().get_file_position())
                ));
            }

            return Err(ParseError::new("Unexpected end of token stream", Some(FilePosition::unknown())));
        }

        let next = self.tokens.peek();

        if next.is_none() {
            self.ntok = None;
        } else {
            self.ntok = next.cloned();
        }

        self.advance_token_index();

        return Ok(());
    }

    fn advance_token_index(&mut self) {
        self.tokidx += 1;
    }
}

impl<'a> AstParser<'a> {
    pub fn new(tokens: &'a [Token]) -> AstParser<'a> {
        return AstParser {
            ctok: None,
            ptok: None,
            ntok: None,
            shadow: &tokens,
            tokidx: 0,
            tokens: tokens.iter().peekable()
        };
    }

    /// Start AST parsing.
    pub fn start_parse(&mut self) -> PR<Ast> {
        {
            match self.tokens.peek() {
                Some(tok) => self.ntok = Some(tok.to_owned()),
                None => return Err(ParseError::new(
                    "Invalid token stream, no tokens found",
                    Some(FilePosition::unknown())
                ))
            };
        }

        let (_, entrypoint) = self.parse_block(true)?;

        return Ok(Ast {
            entrypoint: entrypoint
        });
    }

    /// Parse a curly brace block. `is_root` defines whether we are at a
    /// function body root or whether we are in an inner block, e.g. ifelse
    /// block.
    ///
    /// Returns a tuple, the first item contains idents for the types of
    /// statements the block contains, the other contains the block expression
    /// itself.
    fn parse_block(&mut self, is_root: bool) -> PR<(Vec<&str>, Box<Block>)> {
        let block_open_pos: FilePosition = if !is_root {
            // at root we have no curly bounds
            self.consume_next(vec!["{"])?;

            match self.ctok.unwrap() {
                Token::CurlyOpen(f) => f.to_owned(),
                _ => unreachable!()
            }
        } else {
            self.ntok.unwrap_or(&Token::Eob).get_file_position()
        };

        let statements = self.parse_statements()?;

        if !is_root {
            // at root we have no curly bounds
            self.consume_next(vec!["}"])?;
        }

        // let found_kinds: Vec<&str> = statements.iter().map(|stmt| match stmt.kind {
        //     StatementKind::Return(..) => "return",
        //     StatementKind::Break => "break",
        //     _ => "generic"
        // }).collect();

        let found_kinds = Vec::new(); // FIXME is this needed?

        return Ok((found_kinds, Box::new(Block {
            statements: statements,
            file_position: block_open_pos.to_owned()
        })));
    }

    /// Parse statements inside a block.
    fn parse_statements(&mut self) -> PR<Vec<Box<Statement>>> {
        let mut statements = Vec::new();

        loop {
            // determine which statements end with a `;` character
            let stmt_ends_in_eos = match self.ntok.unwrap() {
                Token::Name(..) | Token::KwVar(..) |
                Token::KwContinue(..) | Token::KwBreak(..) | Token::KwReturn(..) |
                Token::ParensOpen(..) => true,
                _ => false
            };

            let statement: Box<Statement> = match self.ntok.unwrap() {
                Token::Eob | Token::CurlyClose(..) => break,
                Token::KwVar(..) => self.parse_variable_declaration_statement()?,
                Token::KwIf(..) => self.parse_if_statement()?,
                Token::KwLoop(..) => self.parse_loop_statement()?,
                Token::KwFor(..) => self.parse_for_statement()?,
                Token::KwReturn(..) => self.parse_return_statement()?,
                Token::KwBreak(..) => self.parse_break_statement()?,
                Token::KwContinue(..) => self.parse_continue_statement()?,
                _ => self.parse_expression_statement()?,
            };

            if stmt_ends_in_eos {
                self.consume_next(vec![";"])?;
            }

            statements.push(statement);
        }

        return Ok(statements);
    }

    /// Parse a type declaration.
    fn parse_type_declaration(&mut self, parse_param_types: bool) -> PR<Box<SahaType>> {
        self.consume_next(vec!["name", "typestring", "typeboolean", "typeinteger", "typefloat"])?;

        let typ = match self.ctok.unwrap() {
            Token::TypeBoolean(..) => SahaType::Bool,
            Token::TypeString(..) => SahaType::Str,
            Token::TypeInteger(..) => SahaType::Int,
            Token::TypeFloat(..) => SahaType::Float,
            Token::Name(_, n, _) => {
                if parse_param_types && self.validate_paramtype_name(&n) {
                    SahaType::TypeParam(n.to_owned().chars().nth(0).unwrap())
                } else {
                    let mut type_params = Vec::new();

                    match self.ntok.unwrap() {
                        Token::OpLt(..) => {
                            // parse type parameters
                            self.consume_next(vec!["<"])?;

                            'gens: loop {
                                // parse with recursion, in case subtypes are generic as well
                                type_params.push(self.parse_type_declaration(parse_param_types)?);

                                // see if we have a list of comma separated types, then continue
                                // or break the loop
                                match self.ntok.unwrap() {
                                    Token::Comma(..) => {
                                        self.consume_next(vec![","])?;

                                        continue 'gens
                                    },
                                    _ => break 'gens
                                };
                            };

                            self.consume_next(vec![">"])?;

                            SahaType::Name(n.to_owned(), type_params)
                        },
                        _ => {
                            SahaType::Name(n.to_owned(), type_params)
                        }
                    }
                }
            },
            _ => unreachable!()
        };

        return Ok(Box::new(typ));
    }

    /// Parse a variable declaration.
    fn parse_variable_declaration_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["var"])?;

        let statement_pos = match self.ctok.unwrap() {
            Token::KwVar(f) => f,
            _ => unreachable!()
        };

        self.consume_next(vec!["name"])?;

        let (ident_pos, ident_val) = match self.ctok.unwrap() {
            Token::Name(f, _, n) => (f, n),
            _ => unreachable!()
        };

        let identifier = Identifier {
            file_position: ident_pos.to_owned(),
            identifier: ident_val.to_owned(),
            type_params: Vec::new()
        };

        // variable type
        self.consume_next(vec!["'"])?;

        let var_type = self.parse_type_declaration(true)?;

        // if we don't have an assignment we return an uninited variable
        if let Token::EndStatement(..) = self.ntok.unwrap() {
            let stmt = Statement {
                file_position: statement_pos.to_owned(),
                kind: StatementKind::VarDeclaration(identifier, var_type, None),
            };

            return Ok(Box::new(stmt));
        }

        self.consume_next(vec!["="])?;

        let value_expr: Box<Expression> = self.parse_expression(0)?;

        let stmt = Statement {
            file_position: statement_pos.to_owned(),
            kind: StatementKind::VarDeclaration(identifier, var_type, Some(value_expr))
        };

        return Ok(Box::new(stmt));
    }

    /// Parse a statement which is a bare expression.
    fn parse_expression_statement(&mut self) -> PR<Box<Statement>> {
        let stmt = Statement {
            file_position: self.ntok.unwrap().get_file_position(),
            kind: StatementKind::Expression(self.parse_expression(0)?)
        };

        return Ok(Box::new(stmt));
    }

    /// Parse a break statement.
    fn parse_break_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["break"])?;

        return Ok(Box::new(Statement {
            kind: StatementKind::Break,
            file_position: self.ctok.unwrap().get_file_position()
        }));
    }

    /// Parse a break statement.
    fn parse_continue_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["continue"])?;

        return Ok(Box::new(Statement {
            kind: StatementKind::Continue,
            file_position: self.ctok.unwrap().get_file_position()
        }));
    }

    /// Parse a return statement.
    fn parse_return_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["return"])?;

        let return_pos = self.ctok.unwrap().get_file_position();

        // if we encounter a `;` right after the return keyword, we are returning void, otherwise
        // we should parse and expression
        //
        // we don't consume the `;`, as that is handled in statement parent parsing method from
        // where this method was called
        let return_expr: Box<Expression> = match self.ntok.unwrap() {
            Token::EndStatement(..) => Box::new(Expression {
                file_position: self.ntok.unwrap().get_file_position(),
                kind: ExpressionKind::LiteralValue(Value::void())
            }),
            _ => self.parse_expression(0)?
        };

        return Ok(Box::new(Statement {
            file_position: return_pos,
            kind: StatementKind::Return(return_expr)
        }));
    }

    /// Parse if-elseif-else statements.
    fn parse_if_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["if"])?;

        let if_pos = self.ctok.unwrap().get_file_position();

        self.consume_next(vec!["("])?;

        let if_cond = self.parse_expression(0)?;

        self.consume_next(vec![")"])?;

        let (_, if_block) = self.parse_block(false)?;
        let mut elifs: Vec<Box<Statement>> = Vec::new();
        let mut elseblock: Option<Box<Block>> = None;

        while let Token::KwElseif(..) = self.ntok.unwrap() {
            self.consume_next(vec!["elseif"])?;

            let elif_pos = self.ctok.unwrap().get_file_position();

            self.consume_next(vec!["("])?;

            let elif_cond = self.parse_expression(0)?;

            self.consume_next(vec![")"])?;

            let (_, elif_block) = self.parse_block(false)?;

            elifs.push(Box::new(Statement {
                kind: StatementKind::If(elif_cond, elif_block, Vec::new(), None),
                file_position: elif_pos
            }));
        };

        if let Token::KwElse(..) = self.ntok.unwrap() {
            self.consume_next(vec!["else"])?;

            let (_, e) = self.parse_block(false)?;
            elseblock = Some(e);
        }

        let if_statement = Statement {
            kind: StatementKind::If(if_cond, if_block, elifs, elseblock),
            file_position: if_pos
        };

        return Ok(Box::new(if_statement));
    }

    /// Parse a loop statement.
    fn parse_loop_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["loop"])?;

        let loop_pos = self.ctok.unwrap().get_file_position();
        let (_, loop_block) = self.parse_block(false)?;

        return Ok(Box::new(Statement {
            kind: StatementKind::Loop(loop_block),
            file_position: loop_pos
        }));
    }

    /// Parse a for loop statement.
    fn parse_for_statement(&mut self) -> PR<Box<Statement>> {
        self.consume_next(vec!["for"])?;

        let for_pos = self.ctok.unwrap().get_file_position();

        self.consume_next(vec!["("])?;
        self.consume_next(vec!["name"])?;

        let kname = match self.ctok.unwrap() {
            Token::Name(_, _, n) => n.clone(),
            _ => unreachable!()
        };

        let kident = Identifier {
            file_position: self.ctok.unwrap().get_file_position(),
            identifier: kname,
            type_params: Vec::new()
        };

        self.consume_next(vec![","])?;
        self.consume_next(vec!["name"])?;

        let vname = match self.ctok.unwrap() {
            Token::Name(_, _, n) => n.clone(),
            _ => unreachable!()
        };

        let vident = Identifier {
            file_position: self.ctok.unwrap().get_file_position(),
            identifier: vname,
            type_params: Vec::new()
        };

        self.consume_next(vec!["in"])?;

        let iterable_expr = self.parse_expression(0)?;

        self.consume_next(vec![")"])?;

        let for_block = self.parse_block(false)?;

        let stmt = Statement {
            kind: StatementKind::For(kident, vident, iterable_expr, for_block.1),
            file_position: for_pos
        };

        return Ok(Box::new(stmt));
    }

    /// Parse an expression.
    fn parse_expression(&mut self, minimum_op_precedence: i8) -> PR<Box<Expression>> {
        let expression = self.parse_primary()?;

        let ntok: &Token = self.ntok.unwrap_or(&Token::Eob);
        let next_precedence = ntok.get_precedence();

        match ntok {
            Token::ObjectAccess(..) | Token::StaticAccess(..) => {
                return self.parse_generic_object_access(expression);
            },
            _ => {}
        };

        if next_precedence < minimum_op_precedence {
            // non-operator or lesser precedence
            return Ok(expression);
        }

        return self.parse_binop_expression(expression);
    }

    /// Primaries are building blocks for expressions. We could parse these in the
    /// `parse_expression` method, but separating concerns makes it simpler to consume. Also helps
    /// with operator precedence parsing.
    fn parse_primary(&mut self) -> PR<Box<Expression>> {
        self.consume_next(vec![
            "(", "[", "{", "new", "-", "!",
            "name", "stringval", "integerval", "floatval", "booleanval"
        ])?;

        let primary: Box<Expression> = match self.ctok.unwrap() {
            Token::ParensOpen(..) => {
                let expr = self.parse_expression(0)?;

                self.consume_next(vec![")"])?;

                expr
            },
            Token::BraceOpen(..) => self.parse_list_creation_shorthand()?,
            Token::CurlyOpen(..) => self.parse_dict_creation_shorthand()?,
            Token::UnOpNot(..)
            | Token::OpSub(..) => self.parse_unop_expression()?,
            Token::StringValue(..)
            | Token::IntegerValue(..)
            | Token::FloatValue(..)
            | Token::BooleanValue(..) => self.parse_literal_value()?,
            Token::KwNew(..) => self.parse_new_instance_expression()?,
            Token::Name(..) => {
                let identpath_expr = self.parse_ident_path()?;

                // see if we're working with a function call or assigment
                match self.ntok.unwrap() {
                    Token::ParensOpen(..) => self.parse_function_call(identpath_expr)?,
                    Token::Assign(..) => self.parse_assignment_expression(identpath_expr)?,
                    _ => identpath_expr
                }
            },
            _ => unreachable!()
        };

        return Ok(primary);
    }

    /// Parse bracedelimited list creation expression.
    fn parse_list_creation_shorthand(&mut self) -> PR<Box<Expression>> {
        let list_pos = self.ctok.unwrap().get_file_position();
        let mut list_expr: Vec<Box<Expression>> = Vec::new();

        loop {
            list_expr.push(self.parse_expression(0)?);

            match self.ntok.unwrap() {
                Token::Comma(..) => {
                    self.consume_next(vec![","])?;

                    continue
                },
                Token::BraceClose(..) => break,
                _ => continue
            };
        }

        self.consume_next(vec!["]"])?;

        return Ok(Box::new(Expression {
            kind: ExpressionKind::ListDeclaration(list_expr),
            file_position: list_pos
        }));
    }

    /// Parse curlybracedelimited dict creation expression.
    fn parse_dict_creation_shorthand(&mut self) -> PR<Box<Expression>> {
        let dict_pos = self.ctok.unwrap().get_file_position();
        let mut dict_expr: Vec<(Box<Expression>, Box<Expression>)> = Vec::new();

        loop {
            let key = self.parse_expression(0)?;

            self.consume_next(vec![":"])?;

            let value = self.parse_expression(0)?;

            dict_expr.push((key, value));

            match self.ntok.unwrap() {
                Token::Comma(..) => {
                    self.consume_next(vec![","])?;

                    continue
                },
                Token::CurlyClose(..) => break,
                _ => continue
            };
        }

        self.consume_next(vec!["}"])?;

        return Ok(Box::new(Expression {
            kind: ExpressionKind::DictDeclaration(dict_expr),
            file_position: dict_pos
        }));
    }

    /// Parse an assignment expression.
    fn parse_assignment_expression(&mut self, identpath: Box<Expression>) -> PR<Box<Expression>> {
        self.consume_next(vec!["="])?;

        let value_expr = self.parse_expression(0)?;

        return Ok(Box::new(Expression {
            file_position: identpath.file_position.clone(),
            kind: ExpressionKind::Assignment(identpath, value_expr)
        }));
    }

    /// Parse a generic object access expression where some member of something is being
    /// accessed.
    fn parse_generic_object_access(&mut self, lhs_expr: Box<Expression>) -> PR<Box<Expression>> {
        self.consume_next(vec!["->", "::"])?;

        let akind = match self.ctok.unwrap() {
            Token::ObjectAccess(..) => AccessKind::Instance,
            Token::StaticAccess(..) => AccessKind::Static,
            _ => unreachable!()
        };

        let epos = self.ctok.unwrap().get_file_position();
        let rhs_expr = self.parse_expression(0)?;

        let expr = Expression {
            file_position: epos,
            kind: ExpressionKind::ObjectAccess(lhs_expr, akind, rhs_expr)
        };

        return Ok(Box::new(expr));
    }

    /// Parse a binary operation. First we parse the op and then the RHS
    /// expression. Then we check if we should parse another binop.
    fn parse_binop_expression(&mut self, lhs_expr: Box<Expression>) -> PR<Box<Expression>> {
        self.consume_next(vec!["+", "-", "*", "/", "&&", "||", "==", ">", "<", ">=", "<="])?;

        let op_token = self.ctok.unwrap();

        let binop = BinOp::from_token(op_token);

        if binop.is_err() {
            return Err(ParseError::new(
                &format!("Could not parse binary operation type from token `{}`", op_token),
                Some(op_token.get_file_position())
            ));
        }

        let binop = binop.ok().unwrap();

        let mut rhs_expression = self.parse_primary()?;

        let mut ntok: &Token = self.ntok.unwrap_or(&Token::Eob);
        let mut next_precedence = ntok.get_precedence();

        while next_precedence >= op_token.get_precedence() && binop.is_left_assoc
        {
            // while we are on route in binops we dig down on the right side to
            // make precedence work
            rhs_expression = self.parse_binop_expression(rhs_expression)?;

            ntok = self.ntok.unwrap_or(&Token::Eob);
            next_precedence = ntok.get_precedence();
        }

        let binop_expr = Box::new(Expression {
            file_position: lhs_expr.file_position.to_owned(),
            kind: ExpressionKind::BinaryOperation(lhs_expr, binop, rhs_expression)
        });

        if next_precedence < 0 {
            // non-operator
            return Ok(binop_expr);
        }

        return self.parse_binop_expression(binop_expr);
    }

    /// Parse an unary operation.
    fn parse_unop_expression(&mut self) -> PR<Box<Expression>> {
        let (unoppos, unopkind) = match self.ctok.unwrap() {
            Token::UnOpNot(f) => (f, UnaryOpKind::Not),
            Token::OpSub(f) => (f, UnaryOpKind::Minus),
            _ => unreachable!()
        };

        let unop = UnaryOp {
            file_position: unoppos.clone(),
            kind: unopkind
        };

        return Ok(Box::new(Expression {
            file_position: unoppos.clone(),
            kind: ExpressionKind::UnaryOperation(unop, self.parse_primary()?)
        }));
    }

    /// Parse a literal value token.
    fn parse_literal_value(&mut self) -> PR<Box<Expression>> {
        let fpos = self.ctok.unwrap().get_file_position();

        let value = match self.ctok.unwrap() {
            Token::StringValue(_, val) => Value::str(val.to_owned()),
            Token::BooleanValue(_, val) => Value::bool(*val),
            Token::IntegerValue(_, val) => Value::int(*val),
            Token::FloatValue(_, val) => Value::float(*val),
            _ => unreachable!()
        };

        let val_expr = Expression {
            file_position: fpos,
            kind: ExpressionKind::LiteralValue(value)
        };

        return Ok(Box::new(val_expr));
    }

    /// Parse a path of identifiers separated by `->` or `::`.
    fn parse_ident_path(&mut self) -> PR<Box<Expression>> {
        let curtok = self.ctok.unwrap();
        let mut path_items: Vec<(AccessKind, Identifier)> = Vec::new();

        let root: Identifier = match curtok {
            Token::Name(pos, alias, _) => {
                let typeparams: Vec<Box<SahaType>>;

                if let Token::OpLt(..) = self.ntok.unwrap() {
                    typeparams = self.parse_new_instance_type_params()?;
                } else {
                    typeparams = Vec::new();
                }

                Identifier {
                    file_position: pos.clone(),
                    identifier: alias.to_string(),
                    type_params: typeparams
                }
            },
            _ => return Err(ParseError::new(
                &format!("Unexpected `{}`, expected name", curtok), Some(curtok.get_file_position())
            ))
        };

        let mut next_is_access_token: bool = match self.ntok.unwrap() {
            Token::StaticAccess(..) | Token::ObjectAccess(..) => true,
            _ => false
        };

        while next_is_access_token {
            self.consume_next(vec!["->", "::"])?;

            let access_kind: AccessKind = match self.ctok.unwrap() {
                Token::ObjectAccess(..) => AccessKind::Instance,
                Token::StaticAccess(..) => AccessKind::Static,
                _ => unreachable!()
            };

            self.consume_next(vec!["name"])?;

            let item_ident: Identifier = match self.ctok.unwrap() {
                Token::Name(pos, alias, _) => {
                    let typeparams: Vec<Box<SahaType>>;

                    if let Token::OpLt(..) = self.ntok.unwrap() {
                        typeparams = self.parse_new_instance_type_params()?;
                    } else {
                        typeparams = Vec::new();
                    }

                    Identifier {
                        file_position: pos.clone(),
                        identifier: alias.to_string(),
                        type_params: typeparams
                    }
                },
                _ => unreachable!()
            };

            path_items.push((access_kind, item_ident));

            next_is_access_token = match self.ntok.unwrap() {
                Token::StaticAccess(..) | Token::ObjectAccess(..) => true,
                _ => false
            };
        }

        let path_expr = Expression {
            file_position: root.file_position.clone(),
            kind: ExpressionKind::IdentPath(root, path_items)
        };

        return Ok(Box::new(path_expr));
    }

    /// Parse a function call expression which is tied to a identifier path.
    fn parse_function_call(&mut self, ident_expr: Box<Expression>) -> PR<Box<Expression>> {
        self.consume_next(vec!["("])?;

        let call_pos = self.ctok.unwrap().get_file_position();

        // FIXME allow single parameter functions to leave out the parameter name
        let call_args: Box<Expression> = self.parse_callable_args(true)?;

        self.consume_next(vec![")"])?;

        let fn_call_expr = Expression {
            file_position: call_pos,
            kind: ExpressionKind::FunctionCall(ident_expr, call_args)
        };

        return Ok(Box::new(fn_call_expr));
    }

    /// Parse function call arguments that are wrapped in parentheses. Also used
    /// for new instance args.
    fn parse_callable_args(&mut self, allow_unnamed_single_param: bool) -> PR<Box<Expression>> {
        let mut args: Vec<Box<Expression>> = Vec::new();
        let args_pos = self.ctok.unwrap().get_file_position();

        loop {
            match self.ntok.unwrap() {
                Token::ParensClose(..) => {
                    break
                },
                Token::Comma(..) => {
                    self.consume_next(vec![","])?;

                    continue
                },
                _ => {
                    let (is_named_arg, arg_expr) = self.parse_callable_arg()?;

                    args.push(arg_expr);

                    if allow_unnamed_single_param && !is_named_arg {
                        break
                    } else {
                        continue
                    }
                }
            }
        };

        let args_expr = Expression {
            file_position: args_pos,
            kind: ExpressionKind::CallableArgs(args)
        };

        return Ok(Box::new(args_expr));
    }

    /// Parse a single function call argument.
    fn parse_callable_arg(&mut self) -> PR<(bool, Box<Expression>)> {
        // right now we need to see if the next thing is a param name or a value
        // expression, meaning we need to check if the next token is a name
        // and if it is whether the one after that is an assignment token
        let is_named_arg = match self.ntok.unwrap() {
            Token::Name(..) => {
                // we assume there should be an `=` token after the current one
                // if we're working with a named arg
                let assign_pos_token = self.shadow[self.tokidx + 1].clone();

                match assign_pos_token {
                    Token::Assign(..) => true,
                    _ => false
                }
            },
            _ => false
        };

        let mut argname = "".to_string();
        let mut argpos = self.ntok.unwrap().get_file_position();

        if is_named_arg {
            self.consume_next(vec!["name"])?;

            let (newargname, newargpos) = match self.ctok.unwrap() {
                Token::Name(pos, _, name) => (name.clone(), pos.clone()),
                _ => unreachable!()
            };

            argname = newargname;
            argpos = newargpos;

            self.consume_next(vec!["="])?;
        }

        let argvalexpr = self.parse_expression(0)?;

        return Ok((is_named_arg, Box::new(Expression {
            file_position: argpos.clone(),
            kind: ExpressionKind::CallableArg(
                Identifier {
                    file_position: argpos.clone(),
                    identifier: argname.clone(),
                    type_params: Vec::new()
                },
                argvalexpr
            )
        })));
    }

    /// Parse a newup.
    fn parse_new_instance_expression(&mut self) -> PR<Box<Expression>> {
        let newup_pos = self.ctok.unwrap().get_file_position();

        self.consume_next(vec!["name"])?;

        let (cname_pos, cname) = match self.ctok.unwrap() {
            Token::Name(pos, alias, _) => (pos, alias),
            _ => unreachable!()
        };

        let typeparams: Vec<Box<SahaType>>;

        if let Token::OpLt(..) = self.ntok.unwrap() {
            typeparams = self.parse_new_instance_type_params()?;
        } else {
            typeparams = Vec::new();
        }

        self.consume_next(vec!["("])?;

        let newup_args = self.parse_callable_args(false)?;

        self.consume_next(vec![")"])?;

        return Ok(Box::new(Expression {
            file_position: newup_pos,
            kind: ExpressionKind::NewInstance(
                Identifier {
                    file_position: cname_pos.clone(),
                    identifier: cname.clone(),
                    type_params: Vec::new()
                },
                newup_args,
                typeparams
            )
        }));
    }

    /// Validate a parameter type name (should be a single uppercase char).
    fn validate_paramtype_name(&self, name: &str) -> bool {
        if name.len() != 1 {
            return false;
        }

        let acceptable = [
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
            'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
        ];

        return acceptable.contains(&name.chars().nth(0).unwrap());
    }

    /// Parse instance newup type param declarations.
    fn parse_new_instance_type_params(&mut self) -> PR<Vec<Box<SahaType>>> {
        let mut tparams: Vec<Box<SahaType>> = Vec::new();

        self.consume_next(vec!["<"])?;

        loop {
            let ty = self.parse_type_declaration(false)?;

            tparams.push(ty);

            match self.ntok.unwrap() {
                Token::OpGt(..) => {
                    break
                },
                Token::Comma(..) => {
                    self.consume_next(vec![","])?;
                    continue
                },
                _ => {
                    continue
                }
            };
        }

        self.consume_next(vec![">"])?;

        return Ok(tparams);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn testfilepos() -> FilePosition {
        return FilePosition::unknown();
    }

    #[test]
    fn test_empty_is_parsed_correctly() {
        let tokens = vec![
            Token::Eob
        ];

        let mut parser = AstParser::new(&tokens);

        let ast = parser.start_parse();

        if ast.is_err() {
            eprintln!("{:?}", ast.err().unwrap().get_message());
            panic!();
        }

        let ast = ast.ok().unwrap();

        assert_eq!(0, ast.entrypoint.statements.len());
    }

    #[test]
    fn test_variable_declarations_are_parsed_correctly() {
        let tokens = vec![
            Token::KwVar(testfilepos()),
            Token::Name(testfilepos(), "foo".to_string(), "foo".to_string()),
            Token::SingleQuote(testfilepos()),
            Token::TypeString(testfilepos()),
            Token::Assign(testfilepos()),
            Token::StringValue(testfilepos(), "bar".to_string()),
            Token::EndStatement(testfilepos()),
            Token::Eob
        ];

        let mut parser = AstParser::new(&tokens);

        let ast = parser.start_parse();

        if ast.is_err() {
            eprintln!("{:?}", ast.err().unwrap().get_message());
            panic!();
        }

        let ast = ast.ok().unwrap();
        let mut statements = ast.entrypoint.statements.clone();

        assert_eq!(1, statements.len());

        let decl: Box<Statement> = statements.pop().unwrap();

        match decl.kind {
            StatementKind::VarDeclaration(ref ident, ref vartype, ref value) => {
                assert_eq!(Identifier {
                    file_position: testfilepos(),
                    identifier: "foo".to_string(),
                    type_params: Vec::new()
                }, ident.to_owned());

                assert_eq!(Box::new(SahaType::Str), vartype.to_owned());

                assert_eq!(Box::new(Expression {
                    file_position: testfilepos(),
                    kind: ExpressionKind::LiteralValue(Value::str("bar".to_string()))
                }), value.to_owned().unwrap())
            },
            _ => unreachable!()
        }
    }

    #[test]
    fn test_binops_are_parsed() {
        let tokens = vec![
            Token::ParensOpen(testfilepos()),
            Token::IntegerValue(testfilepos(), 1),
            Token::OpAdd(testfilepos()),
            Token::IntegerValue(testfilepos(), 1),
            Token::OpAdd(testfilepos()),
            Token::IntegerValue(testfilepos(), 2),
            Token::OpMul(testfilepos()),
            Token::IntegerValue(testfilepos(), 3),
            Token::OpSub(testfilepos()),
            Token::IntegerValue(testfilepos(), 1),
            Token::ParensClose(testfilepos()),
            Token::EndStatement(testfilepos()),
            Token::Eob
        ];

        // above is
        // (1 + 1 + 2 * 3 - 1);

        let mut parser = AstParser::new(&tokens);

        let ast = parser.start_parse();

        if ast.is_err() {
            eprintln!("{:?}", ast.err().unwrap().get_message());
            panic!();
        }

        let ast = ast.ok().unwrap();
        let mut statements = ast.entrypoint.statements.clone();

        assert_eq!(1, statements.len());

        let stmt = statements.pop().unwrap();

        // hacky, but seems to work, just can't be arsed to write out the actual structure in Rust
        // this will break if a dependency's debug format is changed for instance
        let expected_output = String::from("Expression { file_position: /unknown:0:0, kind: BinaryOperation(Expression \
        { file_position: /unknown:0:0, kind: LiteralValue(Value::Int(1)) }, BinOp::Add, Expression { file_position: \
        /unknown:0:0, kind: BinaryOperation(Expression { file_position: /unknown:0:0, kind: LiteralValue(Value::Int(1)) \
        }, BinOp::Add, Expression { file_position: /unknown:0:0, kind: BinaryOperation(Expression { file_position: \
        /unknown:0:0, kind: BinaryOperation(Expression { file_position: /unknown:0:0, kind: LiteralValue(Value::Int(2)) \
        }, BinOp::Mul, Expression { file_position: /unknown:0:0, kind: LiteralValue(Value::Int(3)) }) }, BinOp::Sub, \
        Expression { file_position: /unknown:0:0, kind: LiteralValue(Value::Int(1)) }) }) }) }");

        match stmt.kind {
            StatementKind::Expression(expr) => {
                assert_eq!(expected_output, format!("{:?}", expr));
            },
            _ => panic!("Unexpected statement kind, expected an expression statement")
        };
    }

    #[test]
    fn test_unary_ops_are_parsed_properly() {
        let tokens = vec![
            Token::ParensOpen(testfilepos()),
            Token::OpSub(testfilepos()),
            Token::IntegerValue(testfilepos(), 5),
            Token::OpAdd(testfilepos()),
            Token::IntegerValue(testfilepos(), 2),
            Token::ParensClose(testfilepos()),
            Token::EndStatement(testfilepos()),
            Token::Eob
        ];

        let mut parser = AstParser::new(&tokens);

        let ast = parser.start_parse();

        if ast.is_err() {
            eprintln!("{:?}", ast.err().unwrap().get_message());
            panic!();
        }

        let ast = ast.ok().unwrap();
        let mut statements = ast.entrypoint.statements.clone();

        assert_eq!(1, statements.len());

        let stmt = statements.pop().unwrap();

        let expected_expr = Box::new(Expression {
            file_position: testfilepos(),
            kind: ExpressionKind::BinaryOperation(
                Box::new(Expression {
                    file_position: testfilepos(),
                    kind: ExpressionKind::UnaryOperation(UnaryOp {
                        file_position: testfilepos(),
                        kind: UnaryOpKind::Minus
                    }, Box::new(Expression {
                        file_position: testfilepos(),
                        kind: ExpressionKind::LiteralValue(Value::int(5))
                    }))
                }),
                BinOp {
                    file_position: testfilepos(),
                    kind: BinOpKind::Add,
                    is_left_assoc: true
                },
                Box::new(Expression {
                    file_position: testfilepos(),
                    kind: ExpressionKind::LiteralValue(Value::int(2))
                })
            )
        });

        match stmt.kind {
            StatementKind::Expression(expr) => {
                assert_eq!(expected_expr, expr);
            },
            _ => panic!("Unexpected statement kind, expected an expression statement")
        };
    }
}
