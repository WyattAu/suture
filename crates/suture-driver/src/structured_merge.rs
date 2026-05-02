#[macro_export]
macro_rules! impl_structured_driver {
    (
        driver = $driver:ident,
        name = $name:literal,
        extensions = [$($ext:literal),+ $(,)?],
        value_ty = $value:ty,

        obj_pat = |$obj_map:ident| $obj_arm:pat,
        arr_pat = |$arr_vec:ident| $arr_arm:pat,

        new_map = $new_map:expr,
        wrap_map = |$wrap_m:ident| $wrap_map_expr:expr,
        wrap_arr = |$wrap_v:ident| $wrap_arr_expr:expr,

        key_set = |$ks_map:ident| $ks_expr:expr,
        map_get = |$mg_map:ident, $mg_key:ident| $mg_expr:expr,
        map_insert = |$mi_map:ident, $mi_key:ident, $mi_val:ident| $mi_expr:expr,

        val_str = |$vs_val:ident| $vs_expr:expr,
        child_path = |$cp_parent:ident, $cp_key:ident| $cp_expr:expr,

        parse_val = |$pv_src:ident| $pv_expr:expr,
        serialize_val = |$sv_val:ident| $sv_expr:expr,

        arrow = $arrow:literal,
    ) => {
        #[allow(clippy::collapsible_match)]
        #[allow(clippy::unnested_or_patterns)]
        impl $driver {
            pub fn new() -> Self {
                Self
            }

            fn diff_values(old: &$value, new: &$value, path: &str) -> Vec<SemanticChange> {
                let mut changes = Vec::new();

                if matches!(old, $obj_arm) && matches!(new, $obj_arm) {
                    let __old_map = match old { $obj_arm => $obj_map, _ => unreachable!() };
                    let __new_map = match new { $obj_arm => $obj_map, _ => unreachable!() };

                    let __old_keys: std::collections::HashSet<_> = { let $ks_map = __old_map; $ks_expr };
                    let __new_keys: std::collections::HashSet<_> = { let $ks_map = __new_map; $ks_expr };

                    for __k in &__old_keys {
                        if !__new_keys.contains(__k) {
                            let __cp = { let $cp_parent = path; let $cp_key = __k; $cp_expr };
                            let __val = { let $mg_map = __old_map; let $mg_key = __k; $mg_expr }.unwrap();
                            changes.push(SemanticChange::Removed {
                                path: __cp,
                                old_value: { let $vs_val = __val; $vs_expr },
                            });
                        }
                    }

                    for __k in &__new_keys {
                        if !__old_keys.contains(__k) {
                            let __cp = { let $cp_parent = path; let $cp_key = __k; $cp_expr };
                            let __val = { let $mg_map = __new_map; let $mg_key = __k; $mg_expr }.unwrap();
                            changes.push(SemanticChange::Added {
                                path: __cp,
                                value: { let $vs_val = __val; $vs_expr },
                            });
                        }
                    }

                    for __k in &__old_keys {
                        if __new_keys.contains(__k) {
                            let __cp = { let $cp_parent = path; let $cp_key = __k; $cp_expr };
                            let __old_val = { let $mg_map = __old_map; let $mg_key = __k; $mg_expr }.unwrap();
                            let __new_val = { let $mg_map = __new_map; let $mg_key = __k; $mg_expr }.unwrap();
                            changes.extend(Self::diff_values(__old_val, __new_val, &__cp));
                        }
                    }
                } else if matches!(old, $arr_arm) && matches!(new, $arr_arm) {
                    let __old_arr = match old { $arr_arm => $arr_vec, _ => unreachable!() };
                    let __new_arr = match new { $arr_arm => $arr_vec, _ => unreachable!() };

                    let __max_len = __old_arr.len().max(__new_arr.len());

                    for __i in 0..__max_len {
                        let __cp = format!("{path}/{__i}");
                        match (__old_arr.get(__i), __new_arr.get(__i)) {
                            (None, Some(__nv)) => {
                                changes.push(SemanticChange::Added {
                                    path: __cp,
                                    value: { let $vs_val = __nv; $vs_expr },
                                });
                            }
                            (Some(__ov), None) => {
                                changes.push(SemanticChange::Removed {
                                    path: __cp,
                                    old_value: { let $vs_val = __ov; $vs_expr },
                                });
                            }
                            (Some(__ov), Some(__nv)) => {
                                changes.extend(Self::diff_values(__ov, __nv, &__cp));
                            }
                            (None, None) => {}
                        }
                    }
                } else if old != new {
                    changes.push(SemanticChange::Modified {
                        path: path.to_string(),
                        old_value: { let $vs_val = old; $vs_expr },
                        new_value: { let $vs_val = new; $vs_expr },
                    });
                }

                changes
            }

            fn merge_values(
                base: &$value,
                ours: &$value,
                theirs: &$value,
            ) -> Result<Option<$value>, DriverError> {
                if matches!(base, $obj_arm)
                    && matches!(ours, $obj_arm)
                    && matches!(theirs, $obj_arm)
                {
                    let __base_map = match base { $obj_arm => $obj_map, _ => unreachable!() };
                    let __ours_map = match ours { $obj_arm => $obj_map, _ => unreachable!() };
                    let __theirs_map =
                        match theirs { $obj_arm => $obj_map, _ => unreachable!() };

                    let __base_keys: std::collections::HashSet<_> = { let $ks_map = __base_map; $ks_expr };
                    let __ours_keys: std::collections::HashSet<_> = { let $ks_map = __ours_map; $ks_expr };
                    let __theirs_keys: std::collections::HashSet<_> = { let $ks_map = __theirs_map; $ks_expr };

                    let __all_keys: std::collections::HashSet<_> = __base_keys
                        .iter()
                        .chain(__ours_keys.iter())
                        .chain(__theirs_keys.iter())
                        .copied()
                        .collect();

                    let mut __merged = $new_map;

                    for __k in &__all_keys {
                        let __in_base = __base_keys.contains(__k);
                        let __in_ours = __ours_keys.contains(__k);
                        let __in_theirs = __theirs_keys.contains(__k);

                        match (__in_base, __in_ours, __in_theirs) {
                            (true, true, false) => {
                                let __v = { let $mg_map = __ours_map; let $mg_key = __k; $mg_expr }.unwrap();
                                { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __v.clone(); $mi_expr };
                            }
                            (true, false, true) => {
                                let __v = { let $mg_map = __theirs_map; let $mg_key = __k; $mg_expr }.unwrap();
                                { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __v.clone(); $mi_expr };
                            }
                            (true, true, true) => {
                                let __bv = { let $mg_map = __base_map; let $mg_key = __k; $mg_expr }.unwrap();
                                let __ov = { let $mg_map = __ours_map; let $mg_key = __k; $mg_expr }.unwrap();
                                let __tv = { let $mg_map = __theirs_map; let $mg_key = __k; $mg_expr }.unwrap();

                                if __ov == __tv {
                                    { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __ov.clone(); $mi_expr };
                                } else if __ov == __bv {
                                    { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __tv.clone(); $mi_expr };
                                } else if __tv == __bv {
                                    { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __ov.clone(); $mi_expr };
                                } else if let Some(__m) =
                                    Self::merge_values(__bv, __ov, __tv)?
                                {
                                    { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __m; $mi_expr };
                                } else {
                                    return Ok(None);
                                }
                            }
                            (false, true, true) => {
                                let __ov = { let $mg_map = __ours_map; let $mg_key = __k; $mg_expr }.unwrap();
                                let __tv = { let $mg_map = __theirs_map; let $mg_key = __k; $mg_expr }.unwrap();
                                if __ov == __tv {
                                    { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __ov.clone(); $mi_expr };
                                } else {
                                    return Ok(None);
                                }
                            }
                            (false, true, false) => {
                                let __v = { let $mg_map = __ours_map; let $mg_key = __k; $mg_expr }.unwrap();
                                { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __v.clone(); $mi_expr };
                            }
                            (false, false, true) => {
                                let __v = { let $mg_map = __theirs_map; let $mg_key = __k; $mg_expr }.unwrap();
                                { let $mi_map = &mut __merged; let $mi_key = __k; let $mi_val = __v.clone(); $mi_expr };
                            }
                            (true, false, false) | (false, false, false) => {}
                        }
                    }

                    Ok(Some({ let $wrap_m = __merged; $wrap_map_expr }))
                } else if matches!(base, $arr_arm)
                    && matches!(ours, $arr_arm)
                    && matches!(theirs, $arr_arm)
                {
                    let __base_arr = match base { $arr_arm => $arr_vec, _ => unreachable!() };
                    let __ours_arr = match ours { $arr_arm => $arr_vec, _ => unreachable!() };
                    let __theirs_arr =
                        match theirs { $arr_arm => $arr_vec, _ => unreachable!() };

                    let __max_len =
                        __base_arr.len().max(__ours_arr.len()).max(__theirs_arr.len());
                    let mut __merged_vec = Vec::new();

                    for __i in 0..__max_len {
                        let __bv = __base_arr.get(__i);
                        let __ov = __ours_arr.get(__i);
                        let __tv = __theirs_arr.get(__i);

                        match (__bv, __ov, __tv) {
                            (None, Some(__o), None) => __merged_vec.push(__o.clone()),
                            (None, None, Some(__t)) => __merged_vec.push(__t.clone()),
                            (None, Some(__o), Some(__t)) => {
                                if __o == __t {
                                    __merged_vec.push(__o.clone());
                                } else {
                                    return Ok(None);
                                }
                            }
                            (None, None, _) => {}
                            (Some(_), Some(__o), None) => __merged_vec.push(__o.clone()),
                            (Some(_), None, Some(__t)) => __merged_vec.push(__t.clone()),
                            (Some(_), None, None) => {}
                            (Some(__b), Some(__o), Some(__t)) => {
                                if __o == __t {
                                    __merged_vec.push(__o.clone());
                                } else if __o == __b {
                                    __merged_vec.push(__t.clone());
                                } else if __t == __b {
                                    __merged_vec.push(__o.clone());
                                } else if let Some(__m) = Self::merge_values(__b, __o, __t)? {
                                    __merged_vec.push(__m);
                                } else {
                                    return Ok(None);
                                }
                            }
                        }
                    }

                    Ok(Some({ let $wrap_v = __merged_vec; $wrap_arr_expr }))
                } else {
                    if ours == theirs {
                        Ok(Some(ours.clone()))
                    } else if ours == base {
                        Ok(Some(theirs.clone()))
                    } else if theirs == base {
                        Ok(Some(ours.clone()))
                    } else {
                        Ok(None)
                    }
                }
            }

            fn format_change(change: &SemanticChange) -> String {
                match change {
                    SemanticChange::Added { path, value } => {
                        format!("  ADDED     {path}: {value}")
                    }
                    SemanticChange::Removed { path, old_value } => {
                        format!("  REMOVED   {path}: {old_value}")
                    }
                    SemanticChange::Modified {
                        path,
                        old_value,
                        new_value,
                    } => {
                        format!("  MODIFIED  {path}: {old_value} $arrow {new_value}")
                    }
                    SemanticChange::Moved {
                        old_path,
                        new_path,
                        value,
                    } => {
                        format!("  MOVED     {old_path} $arrow {new_path}: {value}")
                    }
                }
            }
        }

        impl Default for $driver {
            fn default() -> Self {
                Self::new()
            }
        }

        impl SutureDriver for $driver {
            fn name(&self) -> &str {
                $name
            }

            fn supported_extensions(&self) -> &[&str] {
                &[$($ext),+]
            }

            fn diff(
                &self,
                base_content: Option<&str>,
                new_content: &str,
            ) -> Result<Vec<SemanticChange>, DriverError> {
                let __new_val: $value = { let $pv_src = new_content; $pv_expr }?;

                match base_content {
                    None => {
                        let mut __changes = Vec::new();
                        collect_all_paths(&__new_val, "/".to_string(), &mut __changes);
                        Ok(__changes)
                    }
                    Some(__base) => {
                        let __old_val: $value = { let $pv_src = __base; $pv_expr }?;
                        Ok(Self::diff_values(&__old_val, &__new_val, "/"))
                    }
                }
            }

            fn format_diff(
                &self,
                base_content: Option<&str>,
                new_content: &str,
            ) -> Result<String, DriverError> {
                let __changes = self.diff(base_content, new_content)?;

                if __changes.is_empty() {
                    return Ok("no changes".to_string());
                }

                let __lines: Vec<String> = __changes.iter().map(Self::format_change).collect();
                Ok(__lines.join("\n"))
            }

            fn merge(
                &self,
                base: &str,
                ours: &str,
                theirs: &str,
            ) -> Result<Option<String>, DriverError> {
                let __base_val: $value = { let $pv_src = base; $pv_expr }?;
                let __ours_val: $value = { let $pv_src = ours; $pv_expr }?;
                let __theirs_val: $value = { let $pv_src = theirs; $pv_expr }?;

                match Self::merge_values(&__base_val, &__ours_val, &__theirs_val)? {
                    Some(__merged) => Ok(Some({ let $sv_val = &__merged; $sv_expr }?)),
                    None => Ok(None),
                }
            }
        }

        #[allow(clippy::collapsible_match)]
        fn collect_all_paths(
            __val: &$value,
            __path: String,
            __out: &mut Vec<SemanticChange>,
        ) {
            match __val {
                $obj_arm => {
                    for (__k, __child) in $obj_map {
                        let __cp = { let $cp_parent = &__path; let $cp_key = __k; $cp_expr };
                        collect_all_paths(__child, __cp, __out);
                    }
                }
                $arr_arm => {
                    for (__i, __child) in $arr_vec.iter().enumerate() {
                        let __cp = format!("{__path}/{__i}");
                        collect_all_paths(__child, __cp, __out);
                    }
                }
                __other => {
                    __out.push(SemanticChange::Added {
                        path: __path,
                        value: { let $vs_val = __other; $vs_expr },
                    });
                }
            }
        }
    };
}
