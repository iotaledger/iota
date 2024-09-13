// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    annotations::Annotations,
    stackless_bytecode::{AttrId, Bytecode, Label},
};
use itertools::Itertools;
use move_binary_format::file_format::CodeOffset;
use move_model::{
    model::{DatatypeId, FunId, FunctionEnv, FunctionVisibility, GlobalEnv, Loc, ModuleEnv},
    symbol::{Symbol, SymbolPool},
    ty::{Type, TypeDisplayContext},
};

use crate::function_target_pipeline::FunctionVariant;
use move_model::ast::TempIndex;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::Range,
};

/// A FunctionTarget is a drop-in replacement for a FunctionEnv which allows to
/// rewrite and analyze bytecode and parameter/local types. It encapsulates a
/// FunctionEnv and information which can be rewritten using the
/// `FunctionTargetsHolder` data structure.
pub struct FunctionTarget<'env> {
    pub func_env: &'env FunctionEnv<'env>,
    pub data: &'env FunctionData,

    // Used for debugging and testing, containing any attached annotation formatters.
    annotation_formatters: RefCell<Vec<Box<AnnotationFormatter>>>,
}

impl<'env> Clone for FunctionTarget<'env> {
    fn clone(&self) -> Self {
        // Annotation formatters are transient and forgotten on clone, so this is a
        // cheap handle.
        Self {
            func_env: self.func_env,
            data: self.data,
            annotation_formatters: RefCell::new(vec![]),
        }
    }
}

/// Holds the owned data belonging to a FunctionTarget, contained in a
/// `FunctionTargetsHolder`.
#[derive(Debug, Clone)]
pub struct FunctionData {
    /// The function variant.
    pub variant: FunctionVariant,
    /// The type instantiation.
    pub type_args: Vec<Type>,
    /// The bytecode.
    pub code: Vec<Bytecode>,
    /// The locals, including parameters.
    pub local_types: Vec<Type>,
    /// The return types.
    pub return_types: Vec<Type>,
    /// The set of global resources acquired by  this function.
    pub acquires_global_resources: Vec<DatatypeId>,
    /// A map from byte code attribute to source code location.
    pub locations: BTreeMap<AttrId, Loc>,
    /// The set of asserts that represent loop invariants
    pub loop_invariants: BTreeSet<AttrId>,
    /// A map from byte code attribute to comments associated with this
    /// bytecode. These comments are generated by transformations and are
    /// intended for internal debugging when the bytecode is dumped.
    pub debug_comments: BTreeMap<AttrId, String>,
    /// A map from byte code attribute to a message to be printed out if
    /// verification fails at this bytecode.
    pub vc_infos: BTreeMap<AttrId, String>,
    /// Annotations associated with this function. This is shared between
    /// multiple function variants.
    pub annotations: Annotations,
    /// A mapping from symbolic names to temporaries.
    pub name_to_index: BTreeMap<Symbol, usize>,
    /// The number of ghost type parameters introduced in order to instantiate
    /// related invariants
    pub ghost_type_param_count: usize,
}

impl<'env> FunctionTarget<'env> {
    pub fn new(
        func_env: &'env FunctionEnv<'env>,
        data: &'env FunctionData,
    ) -> FunctionTarget<'env> {
        FunctionTarget {
            func_env,
            data,
            annotation_formatters: RefCell::new(vec![]),
        }
    }

    /// Returns the name of this function.
    pub fn get_name(&self) -> Symbol {
        self.func_env.get_name()
    }

    /// Gets the id of this function.
    pub fn get_id(&self) -> FunId {
        self.func_env.get_id()
    }

    /// Shortcut for accessing the symbol pool.
    pub fn symbol_pool(&self) -> &SymbolPool {
        self.func_env.module_env.symbol_pool()
    }

    /// Shortcut for accessing the module env of this function.
    pub fn module_env(&self) -> &ModuleEnv {
        &self.func_env.module_env
    }

    /// Shortcut for accessing the global env of this function.
    pub fn global_env(&self) -> &GlobalEnv {
        self.func_env.module_env.env
    }

    /// Returns the location of this function.
    pub fn get_loc(&self) -> Loc {
        self.func_env.get_loc()
    }

    /// Returns the location of the bytecode with the given attribute.
    pub fn get_bytecode_loc(&self, attr_id: AttrId) -> Loc {
        if let Some(loc) = self.data.locations.get(&attr_id) {
            loc.clone()
        } else {
            self.func_env.module_env.env.internal_loc()
        }
    }

    /// Returns the debug comment, if any, associated with the given attribute.
    pub fn get_debug_comment(&self, attr_id: AttrId) -> Option<&String> {
        self.data.debug_comments.get(&attr_id)
    }

    /// Returns the verification condition message, if any, associated with the
    /// given attribute.
    pub fn get_vc_info(&self, attr_id: AttrId) -> Option<&String> {
        self.data.vc_infos.get(&attr_id)
    }

    /// Returns true if this function has public, friend, or script visibility.
    pub fn is_exposed(&self) -> bool {
        self.func_env.is_exposed()
    }

    /// Returns the visibility of this function.
    pub fn visibility(&self) -> FunctionVisibility {
        self.func_env.visibility()
    }

    /// Returns true if this function mutates any references (i.e. has &mut
    /// parameters).
    pub fn is_mutating(&self) -> bool {
        self.func_env.is_mutating()
    }

    /// Returns the number of type parameters associated with this function,
    /// this includes both the defined type parameters and the ghost type
    /// parameters.
    ///
    /// NOTE: with the existence of ghost type parameters, the number returned
    /// here can be different from the number returned by `FunctionEnv`.
    pub fn get_type_parameter_count(&self) -> usize {
        self.func_env.get_type_parameters().len() + self.data.ghost_type_param_count
    }

    /// Returns return type at given index.
    pub fn get_return_type(&self, idx: usize) -> &Type {
        &self.data.return_types[idx]
    }

    /// Returns return types of this function.
    pub fn get_return_types(&self) -> &[Type] {
        &self.data.return_types
    }

    /// Returns the number of return values of this function.
    pub fn get_return_count(&self) -> usize {
        self.data.return_types.len()
    }

    /// Return the number of parameters of this function
    pub fn get_parameter_count(&self) -> usize {
        self.func_env.get_parameter_count()
    }

    /// Return an iterator over this function's parameters
    pub fn get_parameters(&self) -> Range<usize> {
        0..self.func_env.get_parameter_count()
    }

    /// Get the name to be used for a local. If the local has a user name, use
    /// that for naming, otherwise generate a unique name.
    pub fn get_local_name(&self, idx: usize) -> Symbol {
        self.func_env.get_local_name(idx)
    }

    /// Return true if this local has a user name.
    pub fn has_local_user_name(&self, idx: usize) -> bool {
        idx < self.get_user_local_count()
    }

    /// Get the index corresponding to a local name. The name must either match
    /// a user name, or have the syntax `$t<N>$`.
    pub fn get_local_index(&self, name: Symbol) -> Option<usize> {
        self.data.name_to_index.get(&name).cloned().or_else(|| {
            let str = self.global_env().symbol_pool().string(name);
            str.strip_prefix("$t").map(|s| s.parse::<usize>().unwrap())
        })
    }

    /// Gets the number of locals of this function, including parameters.
    pub fn get_local_count(&self) -> usize {
        self.data.local_types.len()
    }

    /// Gets the number of user declared locals of this function, excluding
    /// locals which have been introduced by transformations.
    pub fn get_user_local_count(&self) -> usize {
        self.func_env.get_local_count()
    }

    /// Return an iterator over the non-parameter local variables of this
    /// function
    pub fn get_non_parameter_locals(&self) -> Range<usize> {
        self.get_parameter_count()..self.get_local_count()
    }

    /// Returns true if the index is for a temporary, not user declared local.
    pub fn is_temporary(&self, idx: usize) -> bool {
        self.func_env.is_temporary(idx)
    }

    /// Gets the type of the local at index. This must use an index in the range
    /// as determined by `get_local_count`.
    pub fn get_local_type(&self, idx: usize) -> &Type {
        &self.data.local_types[idx]
    }

    /// Gets the bytecode.
    pub fn get_bytecode(&self) -> &[Bytecode] {
        &self.data.code
    }

    /// Gets annotations.
    pub fn get_annotations(&self) -> &'env Annotations {
        &self.data.annotations
    }

    /// Gets acquired resources
    pub fn get_acquires_global_resources(&self) -> &[DatatypeId] {
        &self.data.acquires_global_resources
    }

    /// Gets index of return parameter for a reference input parameter, or None,
    /// if this is not a reference parameter.
    pub fn get_mut_ref_return_index(&self, idx: usize) -> Option<usize> {
        self.get_mut_ref_mapping().get(&idx).cloned()
    }

    /// Returns a map from &mut parameters to the return indices associated with
    /// them *after* &mut instrumentation. By convention, the return values
    /// are appended after the regular function parameters, in the order
    /// they are in the parameter list.
    pub fn get_mut_ref_mapping(&self) -> BTreeMap<TempIndex, usize> {
        let mut res = BTreeMap::new();
        let mut ret_index = self.func_env.get_return_count();
        for idx in 0..self.get_parameter_count() {
            if self.get_local_type(idx).is_mutable_reference() {
                res.insert(idx, ret_index);
                ret_index = usize::saturating_add(ret_index, 1);
            }
        }
        res
    }

    /// Pretty print a bytecode instruction with offset, comments, annotations,
    /// and VC information.
    pub fn pretty_print_bytecode(
        &self,
        label_offsets: &BTreeMap<Label, CodeOffset>,
        offset: usize,
        code: &Bytecode,
    ) -> String {
        let mut texts = vec![];

        // add debug comment
        let attr_id = code.get_attr_id();
        if let Some(comment) = self.get_debug_comment(attr_id) {
            texts.push(format!("     # {}", comment));
        }

        // add annotations
        let annotations = self
            .annotation_formatters
            .borrow()
            .iter()
            .filter_map(|fmt_fun| fmt_fun(self, offset as CodeOffset))
            .map(|s| format!("     # {}", s.replace('\n', "\n     # ").trim()))
            .join("\n");
        if !annotations.is_empty() {
            texts.push(annotations);
        }

        // add vc info
        if let Some(msg) = self.data.vc_infos.get(&attr_id) {
            let loc = self
                .data
                .locations
                .get(&attr_id)
                .cloned()
                .unwrap_or_else(|| self.global_env().unknown_loc());
            texts.push(format!(
                "     # VC: {} {}",
                msg,
                loc.display(self.global_env())
            ));
        }

        // add the instruction itself with offset
        texts.push(format!(
            "{:>3}: {}",
            offset,
            code.display(self, label_offsets)
        ));

        texts.join("\n")
    }
}

impl FunctionData {
    /// Creates new function target data.
    pub fn new(
        func_env: &FunctionEnv<'_>,
        code: Vec<Bytecode>,
        local_types: Vec<Type>,
        return_types: Vec<Type>,
        locations: BTreeMap<AttrId, Loc>,
        acquires_global_resources: Vec<DatatypeId>,
        loop_invariants: BTreeSet<AttrId>,
    ) -> Self {
        let name_to_index = (0..func_env.get_local_count())
            .map(|idx| (func_env.get_local_name(idx), idx))
            .collect();
        FunctionData {
            variant: FunctionVariant::Baseline,
            type_args: vec![],
            code,
            local_types,
            return_types,
            acquires_global_resources,
            locations,
            loop_invariants,
            debug_comments: Default::default(),
            vc_infos: Default::default(),
            annotations: Default::default(),
            name_to_index,
            ghost_type_param_count: 0,
        }
    }

    /// Computes the next available index for AttrId.
    pub fn next_free_attr_index(&self) -> usize {
        self.code
            .iter()
            .map(|b| b.get_attr_id().as_usize())
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Computes the next available index for Label.
    pub fn next_free_label_index(&self) -> usize {
        self.code
            .iter()
            .filter_map(|b| {
                if let Bytecode::Label(_, l) = b {
                    Some(l.as_usize())
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Apply a variable renaming to this data, adjusting internal data
    /// structures.
    pub fn rename_vars<F>(&mut self, _f: &F)
    where
        F: Fn(TempIndex) -> TempIndex,
    {
        // Nothing to do currently.
    }

    /// Fork this function data, without annotations, and mark it as the given
    /// variant.
    pub fn fork(&self, new_variant: FunctionVariant) -> Self {
        assert_ne!(self.variant, new_variant);
        FunctionData {
            variant: new_variant,
            ..self.clone()
        }
    }

    /// Fork this function data with (potentially partial) instantiations.
    pub fn fork_with_instantiation(
        &self,
        env: &GlobalEnv,
        inst: &[Type],
        new_variant: FunctionVariant,
    ) -> Self {
        let type_args = if self.type_args.is_empty() {
            // This is a basic variant wo/ type instantiation, just use the given inst.
            inst.to_vec()
        } else {
            // This is already specialized, specialize it further with inst.
            Type::instantiate_slice(&self.type_args, inst)
        };

        // fix types
        let local_types = Type::instantiate_slice(&self.local_types, inst);
        let return_types = Type::instantiate_slice(&self.return_types, inst);
        let code = self
            .code
            .iter()
            .map(|bc| bc.instantiate(env, inst))
            .collect();

        // construct the new data
        Self {
            variant: new_variant,
            type_args,
            code,
            local_types,
            return_types,

            ..self.clone()
        }
    }

    /// Get the instantiation of this function as a vector of types.
    pub fn get_type_instantiation(&self, _fun_env: &FunctionEnv) -> Vec<Type> {
        self.type_args.clone()
    }
}

// =================================================================================================
// Formatting

/// A function which is called to display the value of an annotation for a given
/// function target at the given code offset. The function is passed the
/// function target and the code offset, and is expected to pick the annotation
/// of its respective type from the function target and for the given code
/// offset. It should return None if there is no relevant annotation.
pub type AnnotationFormatter = dyn Fn(&FunctionTarget<'_>, CodeOffset) -> Option<String>;

impl<'env> FunctionTarget<'env> {
    /// Register a formatter. Each function target processor which introduces
    /// new annotations should register a formatter in order to get is value
    /// printed when a function target is displayed for debugging or
    /// testing.
    pub fn register_annotation_formatter(&self, formatter: Box<AnnotationFormatter>) {
        self.annotation_formatters.borrow_mut().push(formatter);
    }

    /// Tests use this function to register all relevant annotation formatters.
    /// Extend this with new formatters relevant for tests.
    pub fn register_annotation_formatters_for_test(&self) {}
}

impl<'env> fmt::Display for FunctionTarget<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifier = if self.func_env.is_native() {
            "native "
        } else {
            ""
        };
        write!(
            f,
            "{}{}fun {}::{}",
            self.func_env.visibility_str(),
            modifier,
            self.func_env
                .module_env
                .get_name()
                .display(self.symbol_pool()),
            self.get_name().display(self.symbol_pool())
        )?;
        let tparams_count_all = self.get_type_parameter_count();
        let tparams_count_defined = self.func_env.get_type_parameter_count();
        if tparams_count_all != 0 {
            write!(f, "<")?;
            for i in 0..tparams_count_all {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "#{}", i)?;
                if i >= tparams_count_defined {
                    write!(f, "*")?; // denotes a ghost type parameter
                }
            }
            write!(f, ">")?;
        }
        let tctx = TypeDisplayContext::WithEnv {
            env: self.global_env(),
            type_param_names: None,
        };
        let write_decl = |f: &mut fmt::Formatter<'_>, i: TempIndex| {
            let ty = self.get_local_type(i).display(&tctx);
            if self.has_local_user_name(i) {
                write!(
                    f,
                    "$t{}|{}: {}",
                    i,
                    self.get_local_name(i)
                        .display(self.global_env().symbol_pool()),
                    ty
                )
            } else {
                write!(f, "$t{}: {}", i, ty)
            }
        };
        write!(f, "(")?;
        for i in 0..self.get_parameter_count() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write_decl(f, i)?;
        }
        write!(f, ")")?;
        if self.get_return_count() > 0 {
            write!(f, ": ")?;
            if self.get_return_count() > 1 {
                write!(f, "(")?;
            }
            for i in 0..self.get_return_count() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", self.get_return_type(i).display(&tctx))?;
            }
            if self.get_return_count() > 1 {
                write!(f, ")")?;
            }
        }
        if self.func_env.is_native() {
            writeln!(f, ";")?;
        } else {
            writeln!(f, " {{")?;
            for i in self.get_parameter_count()..self.get_local_count() {
                write!(f, "     var ")?;
                write_decl(f, i)?;
                writeln!(f)?;
            }
            let label_offsets = Bytecode::label_offsets(self.get_bytecode());
            for (offset, code) in self.get_bytecode().iter().enumerate() {
                writeln!(
                    f,
                    "{}",
                    self.pretty_print_bytecode(&label_offsets, offset, code)
                )?;
            }
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}
