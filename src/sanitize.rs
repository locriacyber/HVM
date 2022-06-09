
use crate::{language as lang, rulebook::{is_global_name, duplicator}};
use std::collections::{BTreeMap, HashMap};

// Sanitize
// ========

#[allow(dead_code)]
pub struct SanitizedRule {
  pub rule: lang::Rule,
  pub uses: HashMap<String, u64>,
}

// BTree is used here for determinism (HashMap does not maintain
// order among executions)
pub type NameTable = BTreeMap<String, String>;
pub fn create_fresh(rule: &lang::Rule, fresh: &mut dyn FnMut() -> String) -> Result<NameTable, String> {
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
        lang::Term::U32 { .. } => {}
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

pub struct CtxSanitizeTerm<'a> {
  pub uses: &'a mut HashMap<String, u64>,
  pub fresh: &'a mut dyn FnMut() -> String,
}

// Sanitize one term, following the described in main function
pub fn sanitize_term(
  term: &lang::Term,
  lhs: bool,
  tbl: &mut NameTable,
  ctx: &mut CtxSanitizeTerm,
) -> Result<Box<lang::Term>, String> {
  fn rename_erased(name: &mut String, uses: &HashMap<String, u64>) {
    if !is_global_name(name) && uses.get(name).copied() <= Some(0) {
      *name = "*".to_string();
    }
  }
  let term = match term {
    lang::Term::Var { name } => {
      if lhs {
        let mut name = tbl.get(name).unwrap_or(name).clone();
        rename_erased(&mut name, ctx.uses);
        Box::new(lang::Term::Var { name })
      } else if is_global_name(name) {
        if tbl.get(name).is_some() {
          panic!("Using a global variable more than once isn't supported yet. Use an explicit 'let' to clone it. {} {:?}", name, tbl.get(name));
        } else {
          tbl.insert(name.clone(), String::new());
          Box::new(lang::Term::Var { name: name.clone() })
        }
      } else {
        // create a var with the name generated before
        // concatenated with '.{{times_used}}'
        if let Some(name) = tbl.get(name) {
          let used = { *ctx.uses.entry(name.clone()).and_modify(|x| *x += 1).or_insert(1) };
          let name = format!("{}.{}", name, used - 1);
          Box::new(lang::Term::Var { name })
        //} else if is_global_name(&name) {
        // println!("Allowed unbound variable: {}", name);
        // Box::new(lang::Term::Var { name: name.clone() })
        } else {
          return Err(format!("Unbound variable: `{}`.", name));
        }
      }
    }
    lang::Term::Dup { expr, body, nam0, nam1 } => {
      let new_nam0 = (ctx.fresh)();
      let new_nam1 = (ctx.fresh)();
      let expr = sanitize_term(expr, lhs, tbl, ctx)?;
      let got_nam0 = tbl.remove(nam0);
      let got_nam1 = tbl.remove(nam1);
      tbl.insert(nam0.clone(), new_nam0.clone());
      tbl.insert(nam1.clone(), new_nam1.clone());
      let body = sanitize_term(body, lhs, tbl, ctx)?;
      tbl.remove(nam0);
      if let Some(x) = got_nam0 {
        tbl.insert(nam0.clone(), x);
      }
      tbl.remove(nam1);
      if let Some(x) = got_nam1 {
        tbl.insert(nam1.clone(), x);
      }
      let nam0 = format!("{}.0", new_nam0);
      let nam1 = format!("{}.0", new_nam1);
      let term = lang::Term::Dup { nam0, nam1, expr, body };
      Box::new(term)
    }
    lang::Term::Let { name, expr, body } => {
      let new_name = (ctx.fresh)();
      let expr = sanitize_term(expr, lhs, tbl, ctx)?;
      let got_name = tbl.remove(name);
      tbl.insert(name.clone(), new_name.clone());
      let body = sanitize_term(body, lhs, tbl, ctx)?;
      tbl.remove(name);
      if let Some(x) = got_name {
        tbl.insert(name.clone(), x);
      }
      duplicator(&new_name, expr, body, ctx.uses)
    }
    lang::Term::Lam { name, body } => {
      let mut new_name = if is_global_name(name) { name.clone() } else { (ctx.fresh)() };
      let got_name = tbl.remove(name);
      tbl.insert(name.clone(), new_name.clone());
      let body = sanitize_term(body, lhs, tbl, ctx)?;
      tbl.remove(name);
      if let Some(x) = got_name {
        tbl.insert(name.clone(), x);
      }
      let expr = Box::new(lang::Term::Var { name: new_name.clone() });
      let body = duplicator(&new_name, expr, body, ctx.uses);
      rename_erased(&mut new_name, ctx.uses);
      let term = lang::Term::Lam { name: new_name, body };
      Box::new(term)
    }
    lang::Term::App { func, argm } => {
      let func = sanitize_term(func, lhs, tbl, ctx)?;
      let argm = sanitize_term(argm, lhs, tbl, ctx)?;
      let term = lang::Term::App { func, argm };
      Box::new(term)
    }
    lang::Term::Ctr { name, args } => {
      let mut n_args = Vec::with_capacity(args.len());
      for arg in args {
        let arg = sanitize_term(arg, lhs, tbl, ctx)?;
        n_args.push(arg);
      }
      let term = lang::Term::Ctr { name: name.clone(), args: n_args };
      Box::new(term)
    }
    lang::Term::Op2 { oper, val0, val1 } => {
      let val0 = sanitize_term(val0, lhs, tbl, ctx)?;
      let val1 = sanitize_term(val1, lhs, tbl, ctx)?;
      let term = lang::Term::Op2 { oper: *oper, val0, val1 };
      Box::new(term)
    }
    lang::Term::U32 { numb } => {
      let term = lang::Term::U32 { numb: *numb };
      Box::new(term)
    }
  };

  Ok(term)
}
