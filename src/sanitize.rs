/// Sanitize term
use crate::{
  language as lang,
  rulebook::{duplicator, is_global_name},
};
use std::collections::{BTreeMap, HashMap};

#[allow(dead_code)]
pub struct SanitizedRule {
  pub rule: lang::Rule,
  pub uses: HashMap<String, u64>,
}

// BTree is used here for determinism (HashMap does not maintain
// order among executions)
pub type NameTable = BTreeMap<String, String>;
pub fn create_fresh(
  rule: &lang::Rule,
  fresh: &mut dyn FnMut() -> String,
) -> Result<NameTable, String> {
  let mut table = BTreeMap::new();

  let lhs = &rule.lhs;
  if let lang::Term::Ctr { name: _, ref args } = **lhs {
    for arg in args {
      match &**arg {
        lang::Term::Var { name, .. } => {
          table.insert(name.clone(), fresh());
        }
        lang::Term::Ctr { args, .. } => {
          for arg in args {
            if let lang::Term::Var { name } = &**arg {
              table.insert(name.clone(), fresh());
            }
          }
        }
        lang::Term::Const(_) => {}
        _ => {
          return Err("Invalid left-hand side".to_owned());
        }
      }
    }
  } else {
    return Err("Invalid left-hand side".to_owned());
  }

  Ok(table)
}

fn rename_erased(name: &mut String, uses: &HashMap<String, u64>) {
  if !is_global_name(name) && uses.get(name).copied() <= Some(0) {
    *name = "*".to_string();
  }
}

fn duplicate_inplace(body: &mut lang::Term, new_name: &str, expr: &lang::Term, ctx: &CtxSanitizeTerm) {
    *body = duplicator(new_name, expr, body.clone(), ctx.uses)
}


pub struct CtxSanitizeTerm<'a> {
  pub uses: &'a mut HashMap<String, u64>,
  pub fresh: &'a mut dyn FnMut() -> String,
}

pub fn sanitize_term_inplace<'a>(
  term: &'a mut lang::Term,
  lhs: bool,
  tbl: &mut NameTable,
  ctx: &mut CtxSanitizeTerm,
) -> Result<(), String> {
  match term {
    lang::Term::Var { name } => {
      if lhs {
        *name = tbl.get(name).unwrap_or(name).clone();
        rename_erased(name, ctx.uses);
      } else if is_global_name(name) {
        if tbl.get(name).is_some() {
          panic!("Using a global variable more than once isn't supported yet. Use an explicit 'let' to clone it. {} {:?}", name, tbl.get(name));
        } else {
          tbl.insert(name.clone(), String::new());
        }
      } else {
        // create a var with the name generated before
        // concatenated with '.{{times_used}}'
        if let Some(name2) = tbl.get(name) {
          let used = { *ctx.uses.entry(name2.clone()).and_modify(|x| *x += 1).or_insert(1) };
          *name = format!("{}.{}", name2, used - 1);
        //} else if is_global_name(&name) {
        // println!("Allowed unbound variable: {}", name);
        // Box::new(lang::Term::Var { name: name.clone() })
        } else {
          return Err(format!("Unbound variable: `{}`.", name));
        }
      }
    }

    lang::Term::Dup { expr, body, nam0, nam1 } => {
      sanitize_term_inplace(expr, lhs, tbl, ctx)?;

      let new_nam0 = (ctx.fresh)();
      let new_nam1 = (ctx.fresh)();
      let got_nam0 = tbl.remove(nam0);
      let got_nam1 = tbl.remove(nam1);
      tbl.insert(nam0.clone(), new_nam0.clone());
      tbl.insert(nam1.clone(), new_nam1.clone());

      sanitize_term_inplace(body, lhs, tbl, ctx)?;

      tbl.remove(nam0);

      if let Some(x) = got_nam0 {
        tbl.insert(nam0.clone(), x);
      }
      tbl.remove(nam1);
      if let Some(x) = got_nam1 {
        tbl.insert(nam1.clone(), x);
      }
      *nam0 = format!("{}.0", new_nam0);
      *nam1 = format!("{}.0", new_nam1);
    }

    lang::Term::Let { name, expr, body } => {
      sanitize_term_inplace(expr, lhs, tbl, ctx)?;

      let new_name = (ctx.fresh)();
      let got_name = tbl.remove(name);
      tbl.insert(name.clone(), new_name.clone());

      sanitize_term_inplace(body, lhs, tbl, ctx)?;

      tbl.remove(name);
      if let Some(x) = got_name {
        tbl.insert(name.clone(), x);
      }
      duplicate_inplace(body, &new_name, expr, ctx);
    }
    
    lang::Term::Lam { name, body } => {
      let mut new_name = if is_global_name(name) { name.clone() } else { (ctx.fresh)() };
      let got_name = tbl.remove(name);
      tbl.insert(name.clone(), new_name.clone());

      sanitize_term_inplace(body, lhs, tbl, ctx)?;

      tbl.remove(name);
      if let Some(x) = got_name {
        tbl.insert(name.clone(), x);
      }
      let expr = lang::Term::Var { name: new_name.clone() };
      duplicate_inplace(body, &new_name, &expr, ctx);
      rename_erased(&mut new_name, ctx.uses);
      *name = new_name;
    }

    // noop
    lang::Term::App { func, argm } => {
      sanitize_term_inplace(func, lhs, tbl, ctx)?;
      sanitize_term_inplace(argm, lhs, tbl, ctx)?;
    }
    // noop
    lang::Term::Ctr { name: _, args } => {
      for arg in args {
        sanitize_term_inplace(arg, lhs, tbl, ctx)?;
      }
    }
    // noop
    lang::Term::Op2 { oper: _, val0, val1 } => {
      sanitize_term_inplace(val0, lhs, tbl, ctx)?;
      sanitize_term_inplace(val1, lhs, tbl, ctx)?;
    }
    // noop
    lang::Term::Const(_) => {}
  };
  Ok(())
}
