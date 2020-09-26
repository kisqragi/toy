use super::parse::{ Node, NodeKind::* };

#[derive(Debug, PartialEq, Clone)]
pub enum TypeKind {
    Int,
    Ptr,
    _None,   // Default
}

impl Default for TypeKind { 
    fn default() -> Self { TypeKind::_None }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Type {
    pub kind: TypeKind,
    pub base: Option<Box<Type>>,
}

pub fn is_integer(ty: &Type) -> bool {
    ty.kind == TypeKind::Int
}

fn pointer_to(base: Type) -> Type {
    Type {
        kind: TypeKind::Ptr,
        base: Some(Box::new(base)),
    }
}

fn ty_int() -> Type {
    Type {
        kind: TypeKind::Int,
        base: None,
    }
}

pub fn add_type(node: &mut Node) -> Node {

    if let Some(n) = &node.lhs { node.lhs = Some(Box::new(add_type(&mut n.as_ref().clone()))); }
    if let Some(n) = &node.rhs { node.rhs = Some(Box::new(add_type(&mut n.as_ref().clone()))); }
    if let Some(n) = &node.cond { node.cond = Some(Box::new(add_type(&mut n.as_ref().clone()))); }
    if let Some(n) = &node.then { node.then = Some(Box::new(add_type(&mut n.as_ref().clone()))); }
    if let Some(n) = &node.els { node.els = Some(Box::new(add_type(&mut n.as_ref().clone()))); }
    if let Some(n) = &node.init { node.init = Some(Box::new(add_type(&mut n.as_ref().clone()))); }
    if let Some(n) = &node.inc { node.inc = Some(Box::new(add_type(&mut n.as_ref().clone()))); }

    match node.kind {
        Add | Sub | Mul | Div | Assign => {
            node.ty = node.lhs.as_ref().unwrap().ty.clone()
        }
        Equal | Ne | Lt | Le | Var | Num => {
            node.ty = ty_int()
        }
        Addr => node.ty = pointer_to(node.lhs.as_ref().unwrap().ty.clone()),
        Deref => {
            if node.lhs.as_ref().unwrap().ty.kind == TypeKind::Ptr {
                node.ty = *node.lhs.as_ref().unwrap().ty.base.as_ref().unwrap().clone()
            } else {
                node.ty = ty_int()
            }
        }
        _ =>  {} 
    }

    return node.clone();
}