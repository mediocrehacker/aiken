use std::rc::Rc;

use thiserror::Error;

use crate::ast::{DeBruijn, FakeNamedDeBruijn, Name, NamedDeBruijn, Term, Unique};

mod bimap;

#[derive(Debug, Clone, PartialEq, Copy, Eq, Hash)]
struct Level(usize);

#[derive(Error, Debug)]
pub enum Error {
    #[error("Free Unique `{0}`")]
    FreeUnique(Unique),
    #[error("Free Index `{0}`")]
    FreeIndex(DeBruijn),
}

pub struct Converter {
    current_level: Level,
    levels: Vec<bimap::BiMap>,
    current_unique: Unique,
}

impl Converter {
    pub fn new() -> Self {
        Converter {
            current_level: Level(0),
            levels: vec![bimap::BiMap::new()],
            current_unique: Unique::new(0),
        }
    }

    pub fn name_to_named_debruijn(
        &mut self,
        term: &Term<Name>,
    ) -> Result<Term<NamedDeBruijn>, Error> {
        let converted_term = match term {
            Term::Var(Name { text, unique }) => Term::Var(NamedDeBruijn {
                text: text.to_string(),
                index: self.get_index(*unique)?,
            }),
            Term::Delay(term) => Term::Delay(Rc::new(self.name_to_named_debruijn(term)?)),
            Term::Lambda {
                parameter_name,
                body,
            } => {
                self.declare_unique(parameter_name.unique);

                let index = self.get_index(parameter_name.unique)?;

                let name = NamedDeBruijn {
                    text: parameter_name.text.to_string(),
                    index,
                };

                self.start_scope();

                let body = self.name_to_named_debruijn(body)?;

                self.end_scope();

                self.remove_unique(parameter_name.unique);

                Term::Lambda {
                    parameter_name: name,
                    body: Rc::new(body),
                }
            }
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.name_to_named_debruijn(function)?),
                argument: Rc::new(self.name_to_named_debruijn(argument)?),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => Term::Force(Rc::new(self.name_to_named_debruijn(term)?)),
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        };

        Ok(converted_term)
    }

    pub fn name_to_debruijn(&mut self, term: &Term<Name>) -> Result<Term<DeBruijn>, Error> {
        let converted_term = match term {
            Term::Var(Name { unique, .. }) => Term::Var(self.get_index(*unique)?),
            Term::Delay(term) => Term::Delay(Rc::new(self.name_to_debruijn(term)?)),
            Term::Lambda {
                parameter_name,
                body,
            } => {
                self.declare_unique(parameter_name.unique);

                let name = self.get_index(parameter_name.unique)?;

                self.start_scope();

                let body = self.name_to_debruijn(body)?;

                self.end_scope();

                self.remove_unique(parameter_name.unique);

                Term::Lambda {
                    parameter_name: name,
                    body: Rc::new(body),
                }
            }
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.name_to_debruijn(function)?),
                argument: Rc::new(self.name_to_debruijn(argument)?),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => Term::Force(Rc::new(self.name_to_debruijn(term)?)),
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        };

        Ok(converted_term)
    }

    pub fn named_debruijn_to_name(
        &mut self,
        term: &Term<NamedDeBruijn>,
    ) -> Result<Term<Name>, Error> {
        let converted_term = match term {
            Term::Var(NamedDeBruijn { text, index }) => Term::Var(Name {
                text: text.to_string(),
                unique: self.get_unique(*index)?,
            }),
            Term::Delay(term) => Term::Delay(Rc::new(self.named_debruijn_to_name(term)?)),
            Term::Lambda {
                parameter_name,
                body,
            } => {
                self.declare_binder();

                let unique = self.get_unique(parameter_name.index)?;

                let name = Name {
                    text: parameter_name.text.to_string(),
                    unique,
                };

                self.start_scope();

                let body = self.named_debruijn_to_name(body)?;

                self.end_scope();

                Term::Lambda {
                    parameter_name: name,
                    body: Rc::new(body),
                }
            }
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.named_debruijn_to_name(function)?),
                argument: Rc::new(self.named_debruijn_to_name(argument)?),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => Term::Force(Rc::new(self.named_debruijn_to_name(term)?)),
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        };

        Ok(converted_term)
    }

    pub fn debruijn_to_name(&mut self, term: &Term<DeBruijn>) -> Result<Term<Name>, Error> {
        let converted_term = match term {
            Term::Var(index) => {
                let unique = self.get_unique(*index)?;

                Term::Var(Name {
                    text: format!("i_{}", unique),
                    unique,
                })
            }
            Term::Delay(term) => Term::Delay(Rc::new(self.debruijn_to_name(term)?)),
            Term::Lambda {
                parameter_name,
                body,
            } => {
                self.declare_binder();

                let unique = self.get_unique(*parameter_name)?;

                let name = Name {
                    text: format!("i_{}", unique),
                    unique,
                };

                self.start_scope();

                let body = self.debruijn_to_name(body)?;

                self.end_scope();

                Term::Lambda {
                    parameter_name: name,
                    body: Rc::new(body),
                }
            }
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.debruijn_to_name(function)?),
                argument: Rc::new(self.debruijn_to_name(argument)?),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => Term::Force(Rc::new(self.debruijn_to_name(term)?)),
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        };

        Ok(converted_term)
    }

    pub fn named_debruijn_to_debruijn(&mut self, term: &Term<NamedDeBruijn>) -> Term<DeBruijn> {
        match term {
            Term::Var(name) => Term::Var(name.clone().into()),
            Term::Delay(term) => Term::Delay(Rc::new(self.named_debruijn_to_debruijn(term))),
            Term::Lambda {
                parameter_name,
                body,
            } => Term::Lambda {
                parameter_name: parameter_name.clone().into(),
                body: Rc::new(self.named_debruijn_to_debruijn(body)),
            },
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.named_debruijn_to_debruijn(function)),
                argument: Rc::new(self.named_debruijn_to_debruijn(argument)),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => Term::Force(Rc::new(self.named_debruijn_to_debruijn(term))),
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        }
    }

    pub fn debruijn_to_named_debruijn(&mut self, term: &Term<DeBruijn>) -> Term<NamedDeBruijn> {
        match term {
            Term::Var(name) => Term::Var((*name).into()),
            Term::Delay(term) => Term::Delay(Rc::new(self.debruijn_to_named_debruijn(term))),
            Term::Lambda {
                parameter_name,
                body,
            } => Term::Lambda {
                parameter_name: (*parameter_name).into(),
                body: Rc::new(self.debruijn_to_named_debruijn(body)),
            },
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.debruijn_to_named_debruijn(function)),
                argument: Rc::new(self.debruijn_to_named_debruijn(argument)),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => Term::Force(Rc::new(self.debruijn_to_named_debruijn(term))),
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        }
    }

    pub fn fake_named_debruijn_to_named_debruijn(
        &mut self,
        term: &Term<FakeNamedDeBruijn>,
    ) -> Term<NamedDeBruijn> {
        match term {
            Term::Var(name) => Term::Var(name.clone().into()),
            Term::Delay(term) => {
                Term::Delay(Rc::new(self.fake_named_debruijn_to_named_debruijn(term)))
            }
            Term::Lambda {
                parameter_name,
                body,
            } => Term::Lambda {
                parameter_name: parameter_name.clone().into(),
                body: Rc::new(self.fake_named_debruijn_to_named_debruijn(body)),
            },
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.fake_named_debruijn_to_named_debruijn(function)),
                argument: Rc::new(self.fake_named_debruijn_to_named_debruijn(argument)),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => {
                Term::Force(Rc::new(self.fake_named_debruijn_to_named_debruijn(term)))
            }
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        }
    }

    pub fn named_debruijn_to_fake_named_debruijn(
        &mut self,
        term: &Term<NamedDeBruijn>,
    ) -> Term<FakeNamedDeBruijn> {
        match term {
            Term::Var(name) => Term::Var(name.clone().into()),
            Term::Delay(term) => {
                Term::Delay(Rc::new(self.named_debruijn_to_fake_named_debruijn(term)))
            }
            Term::Lambda {
                parameter_name,
                body,
            } => Term::Lambda {
                parameter_name: parameter_name.clone().into(),
                body: Rc::new(self.named_debruijn_to_fake_named_debruijn(body)),
            },
            Term::Apply { function, argument } => Term::Apply {
                function: Rc::new(self.named_debruijn_to_fake_named_debruijn(function)),
                argument: Rc::new(self.named_debruijn_to_fake_named_debruijn(argument)),
            },
            Term::Constant(constant) => Term::Constant(constant.clone()),
            Term::Force(term) => {
                Term::Force(Rc::new(self.named_debruijn_to_fake_named_debruijn(term)))
            }
            Term::Error => Term::Error,
            Term::Builtin(builtin) => Term::Builtin(*builtin),
        }
    }

    fn get_index(&mut self, unique: Unique) -> Result<DeBruijn, Error> {
        for scope in self.levels.iter().rev() {
            if let Some(found_level) = scope.get(&unique) {
                let index = self.current_level.0 - found_level.0;

                return Ok(index.into());
            }
        }

        Err(Error::FreeUnique(unique))
    }

    fn get_unique(&mut self, index: DeBruijn) -> Result<Unique, Error> {
        for scope in self.levels.iter().rev() {
            let index = Level(self.current_level.0 - index.inner());

            if let Some(unique) = scope.get_right(&index) {
                return Ok(*unique);
            }
        }

        Err(Error::FreeIndex(index))
    }

    fn declare_unique(&mut self, unique: Unique) {
        let scope = &mut self.levels[self.current_level.0];

        scope.insert(unique, self.current_level);
    }

    fn remove_unique(&mut self, unique: Unique) {
        let scope = &mut self.levels[self.current_level.0];

        scope.remove(unique, self.current_level);
    }

    fn declare_binder(&mut self) {
        let scope = &mut self.levels[self.current_level.0];

        scope.insert(self.current_unique, self.current_level);

        self.current_unique.increment();
    }

    fn start_scope(&mut self) {
        self.current_level = Level(self.current_level.0 + 1);

        self.levels.push(bimap::BiMap::new());
    }

    fn end_scope(&mut self) {
        self.current_level = Level(self.current_level.0 - 1);

        self.levels.pop();
    }
}
