use super::tokenize::{ Token, TokenKind };
use super::types::{ Type, add_type, is_integer, ty_int, pointer_to, func_type, copy_type };
use std::process::exit;

#[derive(Debug, PartialEq, Clone)]
pub enum NodeKind {
    Num,        // Integer
    Add,        // +
    Sub,        // -
    Mul,        // *
    Div,        // /
    Equal,      // ==
    Ne,         // !=
    Lt,         // <
    Le,         // <=
    If,         // "if"
    For,        // "for"
    Block,      // { ... }
    ExprStmt,   // Expression statement
    Return,     // Return statement
    Assign,     // =
    Addr,       // &
    Deref,      // *
    Var,        // Variable
    Funcall,    // Function call
    Null,       // Default value of NodeKind
}

impl Default for NodeKind {
    fn default() -> Self { NodeKind::Null }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Node {
    pub kind: NodeKind,             // Node kind
    pub ty: Type,                   // Type, e.g. int or pointer to int

    pub lhs: Option<Box<Node>>,     // Left-hand side
    pub rhs: Option<Box<Node>>,     // Right-hand side

    // "if" or "for" statement
    pub cond: Option<Box<Node>>,
    pub then: Option<Box<Node>>,
    pub els: Option<Box<Node>>,
    pub init: Option<Box<Node>>,
    pub inc: Option<Box<Node>>,

    // Block
    pub body: Option<Vec<Box<Node>>>,

    // Function call
    pub funcname: String,
    pub args: Option<Vec<Box<Node>>>,

    pub var: Option<usize>,         // Used if kind == NodeKind::Var
    pub val: i64,                   // Used if kind == NodeKind::Num
}

#[derive(Debug, Default, Clone)]
pub struct Var {
    pub name: String,
    pub ty: Type,
    pub offset: usize,
}

fn find_var(pc: &mut ParseContext) -> Option<usize> {
    for (i, var) in pc.locals.iter().enumerate() {
        if pc.tokens[pc.pos].s == var.name {
            return Some(i);
        }
    }
    None 
}

fn new_binary(kind: NodeKind, lhs: Box<Node>, rhs: Box<Node>) -> Node {
    Node {
        kind,
        lhs: Some(lhs),
        rhs: Some(rhs),
        ..Default::default()
    }
}

fn new_unary(kind: NodeKind, expr: Box<Node>) -> Node {
    Node {
        kind,
        lhs: Some(expr),
        ..Default::default()
    }
}

fn get_number(val: i64) -> Node {
    Node {
        kind: NodeKind::Num,
        val,
        ..Default::default()
    }
}

fn new_num(pc: &mut ParseContext) -> Node {
    if pc.tokens[pc.pos].kind == TokenKind::Num {
        let val = pc.tokens[pc.pos].val;
        return get_number(val);
    }
    panic!("number expected, but got {}", pc.tokens[pc.pos].s);
}

fn new_var_node(var: usize) -> Node {
    Node {
        kind: NodeKind::Var,
        var: Some(var),
        ..Default::default()
    }
}

fn new_lvar_parms(pc: &mut ParseContext, t: Type) {
    let ty = t.clone();
    let name = t.name.unwrap().s.clone();

    let v = Var {
        name,
        ty,
        ..Default::default()
    };
    pc.locals.push(v);
}

fn new_lvar(pc: &mut ParseContext, ty: Type) -> usize {
    let v = Var {
        name: pc.tokens[pc.pos].s.clone(),
        ty,
        ..Default::default()
    };
    pc.locals.push(v);
    return pc.locals.len()-1;
}


// stmt = "return" expr ";"
//      | "{" compound-stmt
//      | "if" "(" expr ")" stmt ("else" stmt)?
//      | "for" "(" expr? ";" expr? ";" expr? ")" stmt
//      | "while" "(" expr ")" stmt
//      | expr-stmt
fn stmt(pc: &mut ParseContext) -> Node {
    if pc.tokens[pc.pos].s == "return" {
        pc.pos += 1;
        let lhs = expr(pc);
        let node = new_unary(NodeKind::Return, Box::new(lhs));
        skip(pc, ";");
        return node;
    }

    // "if" statement
    if pc.tokens[pc.pos].s == "if" {
        let mut node = Node { kind: NodeKind::If, ..Default::default() };

        pc.pos += 1;
        skip(pc, "(");

        // set cond
        let cond = expr(pc);
        node.cond = Some(Box::new(cond));

        skip(pc, ")");

        // set then 
        let then = stmt(pc);
        node.then = Some(Box::new(then));

        // "else"
        if pc.tokens[pc.pos].s == "else" {
            pc.pos += 1;
            let t = stmt(pc);
            node.els = Some(Box::new(t));
        }

        return node;
    }

    // "for" statement
    if pc.tokens[pc.pos].s == "for" {
        let mut node = Node { kind: NodeKind::For, ..Default::default() };

        pc.pos += 1;
        skip(pc, "(");

        // initとincは値を返さない
        // init
        if pc.tokens[pc.pos].s != ";" {
            let init = expr(pc);
            node.init = Some(Box::new(new_unary(NodeKind::ExprStmt, Box::new(init))));
        }
        skip(pc, ";");

        // cond 
        if pc.tokens[pc.pos].s != ";" {
            let cond = expr(pc);
            node.cond = Some(Box::new(cond));
        }
        skip(pc, ";");

        if pc.tokens[pc.pos].s != ")" {
            let inc = expr(pc);
            node.inc = Some(Box::new(new_unary(NodeKind::ExprStmt, Box::new(inc))));
        }
        skip(pc, ")");

        let then = stmt(pc);
        node.then = Some(Box::new(then));

        return node;
    }

    if pc.tokens[pc.pos].s == "while" {
        let mut node = Node { kind: NodeKind::For, ..Default::default() };
        pc.pos += 1;
        skip(pc, "(");

        let cond = expr(pc);
        node.cond = Some(Box::new(cond));
        skip(pc, ")");

        let then = stmt(pc);
        node.then = Some(Box::new(then));

        return node;
    }

    if pc.tokens[pc.pos].s == "{" {
        pc.pos += 1;
        let body = compound_stmt(pc);
        return body;
    }

    expr_stmt(pc)
}

// compound-stmt = (declaration | stmt)* "}"
fn compound_stmt(pc: &mut ParseContext) -> Node {
    let mut node = Node { kind: NodeKind::Block, ..Default::default() };

    let mut body: Vec<Box<Node>> = vec![];
    while pc.tokens[pc.pos].s != "}" {
        if pc.tokens[pc.pos].s == "int" {
            let mut node = declaration(pc);
            body.push(Box::new(add_type(&mut node)));
        } else {
            let mut node = stmt(pc);
            body.push(Box::new(add_type(&mut node)));
        }
    }

    node.body = Some(body);
    pc.pos += 1;
    return node;
}

// funcdef = typespec declarator "{" compound-stmt
fn funcdef(pc: &mut ParseContext) -> Function {
    pc.locals = Vec::new();
    let ty = typespec(pc);
    let ty = declarator(pc, ty);

    skip(pc, "{");

    for t in ty.params {
        new_lvar_parms(pc, t);
    }
    let params = pc.locals.clone();

    let node = compound_stmt(pc);
    let locals = pc.locals.clone();

    Function {
        name: ty.name.unwrap().s,
        node,
        params,
        locals,
        ..Default::default()
    }
}

// declaration = typespec (declarator ("=" expr)? ("," declarator ("=" expr)?)*)? ";"
fn declaration(pc: &mut ParseContext) -> Node {
    let basety = typespec(pc);

    let mut body: Vec<Box<Node>> = vec![];
    let mut i = 0;
    loop {
        if pc.tokens[pc.pos].s == ";" {
            skip(pc, ";");
            break;
        }

        if i > 0 {
            skip(pc, ",");
        }
        i += 1;

        let ty = declarator(pc, basety.clone());
        pc.pos -= 1;
        let var = new_lvar(pc, ty);

        pc.pos += 1;
        if pc.tokens[pc.pos].s != "=" { continue; }

        let lhs = new_var_node(var);
        pc.pos += 1;
        let rhs = assign(pc);
        let node = new_binary(NodeKind::Assign, Box::new(lhs), Box::new(rhs));
        body.push(Box::new(new_unary(NodeKind::ExprStmt, Box::new(node))));
    }


    let mut node = Node { kind: NodeKind::Block, ..Default::default() };
    node.body = Some(body);
    node
}

// typespec = "int"
fn typespec(pc: &mut ParseContext) -> Type {
    skip(pc, "int");
    ty_int()
}

// declarator = "*"* ident type-suffix
fn declarator(pc: &mut ParseContext, mut ty: Type) -> Type {
    loop {
        if !consume(pc, "*") { break; }
        ty = pointer_to(ty);
    }

    if pc.tokens[pc.pos].kind != TokenKind::Ident {
        eprintln!("expected a variable name: {}", pc.tokens[pc.pos].s);
    }

    let pos = pc.pos;
    let name = Some(pc.tokens[pos].clone());
    pc.pos += 1;
    let mut ty = type_suffix(pc, ty);
    ty.name = name;
    ty
}

// type-suffix = ( "(" func-params? ")" )
// func-params = param ("," param)*
// param       = typespec declarator
fn type_suffix(pc: &mut ParseContext, mut ty: Type) -> Type {
    if pc.tokens[pc.pos].s == "(" {
        pc.pos += 1;

        let mut params: Vec<Type> = vec![];

        while pc.tokens[pc.pos].s != ")" {
            if params.len() != 0 {
                skip(pc, ",");
            }
            let basety = typespec(pc);
            let ty = declarator(pc, basety);
            params.push(copy_type(ty));
        }

        ty = func_type(ty);
        ty.params = params;

        skip(pc, ")");
        return ty;
    }
    ty
}

// expr-stmt = expr ";"
fn expr_stmt(pc: &mut ParseContext) -> Node {
    let lhs = expr(pc);
    let node = new_unary(NodeKind::ExprStmt, Box::new(lhs));
    skip(pc, ";");
    node
}

// expr =  assign
fn expr(pc: &mut ParseContext) -> Node {
    assign(pc)
}

// assign = equality ("=" assign)?
fn assign(pc: &mut ParseContext) -> Node {
    let mut node = equality(pc);
    let op = &pc.tokens[pc.pos].s;
    if op == "=" {
        pc.pos += 1;
        let rhs = assign(pc);
        node = new_binary(NodeKind::Assign, Box::new(node), Box::new(rhs));  
    }

    node
}

// equality = relational ("==" relational | "!=" relational)*
fn equality(pc: &mut ParseContext) -> Node {
    let mut node = relational(pc);

    loop {

        if pc.tokens.len() == pc.pos {
            return node;
        }
        let op = &pc.tokens[pc.pos].s;

        if op == "==" {
            pc.pos += 1;
            let rhs = relational(pc);
            node = new_binary(NodeKind::Equal, Box::new(node), Box::new(rhs));
            continue;
        }

        if op == "!=" {
            pc.pos += 1;
            let rhs = relational(pc);
            node = new_binary(NodeKind::Ne, Box::new(node), Box::new(rhs));
            continue;
        }

        return node;
    }

}

// relational = add ("<" add | "<=" add | ">" add | ">=" add)*
fn relational(pc: &mut ParseContext) -> Node {
    let mut node = add(pc);

    loop {

        if pc.tokens.len() == pc.pos {
            return node;
        }
        let op = &pc.tokens[pc.pos].s;

        if op == "<" {
            pc.pos += 1;
            let rhs = add(pc);
            node = new_binary(NodeKind::Lt, Box::new(node), Box::new(rhs));
            continue;
        }

        if op == "<=" {
            pc.pos += 1;
            let rhs = add(pc);
            node = new_binary(NodeKind::Le, Box::new(node), Box::new(rhs));
            continue;
        }

        if op == ">" {
            pc.pos += 1;
            let rhs = add(pc);
            node = new_binary(NodeKind::Lt, Box::new(rhs), Box::new(node));
            continue;
        }

        if op == ">=" {
            pc.pos += 1;
            let rhs = add(pc);
            node = new_binary(NodeKind::Le, Box::new(rhs), Box::new(node));
            continue;
        }

        return node;
    } }

fn new_add(mut lhs: Node, mut rhs: Node) -> Node {
    lhs = add_type(&mut lhs);
    rhs = add_type(&mut rhs);

    // num + num
    if is_integer(&lhs.ty) && is_integer(&rhs.ty) {
        return new_binary(NodeKind::Add, Box::new(lhs.clone()), Box::new(rhs.clone()));
    }

    if lhs.ty.base != None && rhs.ty.base != None {
        eprintln!("invalid operands");
        eprintln!("lhs = {:#?}", lhs);
        eprintln!("rhs = {:#?}", rhs);
    }

    // Canonicalize `num + ptr` to `ptr + num`.
    if lhs.ty.base == None && rhs.ty.base != None {
        let tmp = lhs;
        lhs = rhs;
        rhs = tmp;
    }

    // ptr + num
    rhs = new_binary(NodeKind::Mul, Box::new(rhs), Box::new(get_number(8)));
    new_binary(NodeKind::Add, Box::new(lhs), Box::new(rhs))
}

fn new_sub(mut lhs: Node, mut rhs: Node) -> Node {
    lhs = add_type(&mut lhs);
    rhs = add_type(&mut rhs);

    // num - num
    if is_integer(&lhs.ty) && is_integer(&rhs.ty) {
        return new_binary(NodeKind::Sub, Box::new(lhs.clone()), Box::new(rhs.clone()));
    }

    // ptr - num
    if lhs.ty.base != None && is_integer(&rhs.ty) {
        rhs = new_binary(NodeKind::Mul, Box::new(rhs), Box::new(get_number(8)));
        return new_binary(NodeKind::Sub, Box::new(lhs), Box::new(rhs));
    }

    // num - ptr (error)
    if lhs.ty.base == None && rhs.ty.base != None {
        eprintln!("invalid operands");
        eprintln!("lhs = {:#?}", lhs);
        eprintln!("rhs = {:#?}", rhs);
    }

    // `ptr-ptr` returns the result of `ptr-ptr` divided by its size.
    // The result is a number of elements, but the value can also be negative.
    lhs = new_binary(NodeKind::Sub, Box::new(lhs), Box::new(rhs));
    new_binary(NodeKind::Div, Box::new(lhs), Box::new(get_number(8)))
}

// add = mul ("+" mul | "-" mul)*
fn add(pc: &mut ParseContext) -> Node {
    let mut node = mul(pc);

    loop {

        if pc.tokens.len() == pc.pos {
            return node;
        }
        let op = &pc.tokens[pc.pos].s;

        if op == "+" {
            pc.pos += 1;
            let rhs = mul(pc);
            node = new_add(node, rhs);
            continue;
        }

        if op == "-" {
            pc.pos += 1;
            let rhs = mul(pc);
            node = new_sub(node, rhs);
            continue;
        }

        return node;
    }
}

// mul = unary ("*" unary | "/" unary)*
fn mul(pc: &mut ParseContext) -> Node {
    let mut node = unary(pc);

    
    loop {
        if pc.tokens.len() == pc.pos {
            return node;
        }

        let op = &pc.tokens[pc.pos].s;
        if op == "*" {
            pc.pos += 1;
            let rhs = unary(pc);
            node = new_binary(NodeKind::Mul, Box::new(node), Box::new(rhs));
            continue;
        }

        if op == "/" {
            pc.pos += 1;
            let rhs = unary(pc);
            node = new_binary(NodeKind::Div, Box::new(node), Box::new(rhs));
            continue;
        }

        return node;
    }
}

// unary = ("+" | "-" | "&" | "*")? unary
//       | primary
fn unary(pc: &mut ParseContext) -> Node {
    match pc.tokens[pc.pos].s.as_str() {
        "+" => {
            pc.pos += 1;
            return unary(pc);
        }
        "-" => {
            pc.pos += 1;
            let node = unary(pc);
            return new_binary(NodeKind::Sub, Box::new(get_number(0)), Box::new(node));
        }
        "&" => {
            pc.pos += 1;
            let node = unary(pc);
            return new_unary(NodeKind::Addr, Box::new(node));
        }
        "*" => {
            pc.pos += 1;
            let node = unary(pc);
            return new_unary(NodeKind::Deref, Box::new(node));
        }
        _ => return primary(pc)
    };
}

// primary   = "(" expr ")" | ident func-args? | num
fn primary(pc: &mut ParseContext) -> Node {
    let c = &pc.tokens[pc.pos].s;
    if c == "(" {
        pc.pos += 1;
        let node = expr(pc);
        skip(pc, ")");
        return node;
    }

    if pc.tokens[pc.pos].kind == TokenKind::Ident {
        // Function call
        if pc.tokens[pc.pos+1].s == "(" {
            return funcall(pc);
        }

        // Variable
        let var = find_var(pc);
        if var == None {
            eprintln!("undefined variable: {}", pc.tokens[pc.pos].s);
            exit(1);
        }
        pc.pos += 1;
        return new_var_node(var.unwrap());
    }

    let node = new_num(pc);
    pc.pos += 1;
    node
}

// func-args = "(" (assign ("," assign)*)? ")"
fn funcall(pc: &mut ParseContext) -> Node {
    let start = pc.pos;
    pc.pos += 2;   // eat ident & "("

    let mut args: Vec<Box<Node>> = vec![];

    while pc.tokens[pc.pos].s != ")" {
        if (pc.pos-2) != start {
            skip(pc, ",");
        }
        let mut node = assign(pc);
        args.push(Box::new(add_type(&mut node)));
    }

    skip(pc, ")");

    let node = Node {
        kind: NodeKind::Funcall,
        funcname: pc.tokens[start].s.clone(),
        args: Some(args),
        ..Default::default()
    };

    node
}

fn skip(pc: &mut ParseContext, s: &str){
    if pc.tokens[pc.pos].s != s {
        panic!("expected '{}'", s);
    }
    pc.pos += 1
}

// トークンが期待するトークンの場合、トークンを一つ消費して
// 真を返す。違う場合偽を返す。
fn consume(pc: &mut ParseContext, s: &str) -> bool {
    if pc.tokens[pc.pos].s == s {
        pc.pos += 1;
        return true;
    }
    false
}


#[derive(Debug, Default, Clone)]
pub struct Function {
    pub name: String,
    pub node: Node,
    pub locals: Vec<Var>,
    pub params: Vec<Var>,
    pub stack_size: usize,
}

#[derive(Debug, Default)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Default)]
struct ParseContext {
    tokens: Vec<Token>,
    pos: usize,
    locals: Vec<Var>,
}

// program = funcdef*
pub fn parse(tokens: Vec<Token>) -> Program {
    let mut prog = Program { ..Default::default() };
    let mut pc = ParseContext { tokens, ..Default::default() };
    while pc.tokens[pc.pos].kind != TokenKind::Eof {
        let func = funcdef(&mut pc);
        prog.functions.push(func);
    }
    prog
}
