/// Sanitize term


use crate::{language as lang, rulebook::{is_global_name, duplicator}};
use std::collections::{BTreeMap, HashMap};

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

pub fn sanitize_term_inplace(
    term: &mut lang::Term,
    lhs: bool,
    tbl: &mut NameTable,
    ctx: &mut CtxSanitizeTerm,
) -> Result<(), String> {
    todo!()
}
