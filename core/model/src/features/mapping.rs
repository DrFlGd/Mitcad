// SPDX-License-Identifier: MIT
//! `map_params!` (mitcad#586): the `map_params` of a definition whose value
//! fields map under their own names, written from a list of its fields.

/// Writes a definition's `map_params` from the list of all its fields, each
/// with how it maps:
///
/// - `clone`: cloned (`self.field.clone()`);
/// - `copy`: copied (`self.field`);
/// - `param`: a value, slot `"field"` (`f("field", &self.field)?`);
/// - `option`: an optional value, slot `"field"`;
/// - `map`: a type of its own, by its `map_params` (`self.field.map_params(f)?`);
/// - `with(helper)`: by a function (`helper(&self.field, f)?`);
/// - `phantom`: `PhantomData`.
///
/// `f` gets the values in the order the fields are listed, which
/// parameter numbering and saved documents depend on: keep it. A mapping
/// with a slot of another name, a prefix or anything else stays written by
/// hand.
///
/// ```text
/// impl<P> BoxDef<P> {
///     map_params! {
///         BoxDef {
///             plane: clone,
///             corner: copy,
///             length: param,
///             angle: option,
///             transform: map,
///             offset: with(map_offset),
///             values: phantom,
///         }
///     }
/// }
/// ```
macro_rules! map_params {
    (@field $s:tt, $f:ident, $field:ident, clone) => {
        $s.$field.clone()
    };
    (@field $s:tt, $f:ident, $field:ident, copy) => {
        $s.$field
    };
    (@field $s:tt, $f:ident, $field:ident, param) => {
        $f(stringify!($field), &$s.$field)?
    };
    (@field $s:tt, $f:ident, $field:ident, option) => {
        $s.$field
            .as_ref()
            .map(|value| $f(stringify!($field), value))
            .transpose()?
    };
    (@field $s:tt, $f:ident, $field:ident, map) => {
        $s.$field.map_params($f)?
    };
    (@field $s:tt, $f:ident, $field:ident, with($helper:path)) => {
        $helper(&$s.$field, $f)?
    };
    (@field $s:tt, $f:ident, $field:ident, phantom) => {
        ::std::marker::PhantomData
    };
    (
        $name:ident {
            $($field:ident: $kind:ident $(($helper:path))?),* $(,)?
        }
    ) => {
        /// Converts the value fields; `f` gets each field's slot and value.
        // A definition without values never calls `f`.
        #[allow(unused_variables)]
        pub fn map_params<Q, E>(
            &self,
            f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
        ) -> Result<$name<Q>, E> {
            Ok($name {
                $($field: $crate::features::mapping::map_params!(
                    @field self, f, $field, $kind $(($helper))?
                ),)*
            })
        }
    };
}
pub(crate) use map_params;

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    /// A part with values of its own.
    #[derive(Debug, PartialEq)]
    struct Inner<P> {
        x: P,
    }

    impl<P> Inner<P> {
        fn map_params<Q, E>(
            &self,
            f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
        ) -> Result<Inner<Q>, E> {
            Ok(Inner {
                x: f("inner.x", &self.x)?,
            })
        }
    }

    /// Values mapped by a function, each with its own slot.
    fn each<P, Q, E>(
        values: &[P],
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<Vec<Q>, E> {
        values
            .iter()
            .enumerate()
            .map(|(i, value)| f(&format!("pair.{i}"), value))
            .collect()
    }

    /// Every kind of field.
    #[derive(Debug, PartialEq)]
    struct Example<P> {
        name: String,
        count: u32,
        length: P,
        angle: Option<P>,
        offset: Option<P>,
        inner: Inner<P>,
        pair: Vec<P>,
        marker: PhantomData<P>,
        width: P,
    }

    impl<P> Example<P> {
        map_params! {
            Example {
                name: clone,
                count: copy,
                length: param,
                angle: option,
                offset: option,
                inner: map,
                pair: with(each),
                marker: phantom,
                width: param,
            }
        }
    }

    /// No values at all.
    #[derive(Debug, PartialEq)]
    struct NoValues<P> {
        bodies: Vec<u32>,
        values: PhantomData<P>,
    }

    impl<P> NoValues<P> {
        map_params! {
            NoValues {
                bodies: clone,
                values: phantom,
            }
        }
    }

    fn example() -> Example<f64> {
        Example {
            name: "a".to_owned(),
            count: 3,
            length: 1.0,
            angle: Some(2.0),
            offset: None,
            inner: Inner { x: 4.0 },
            pair: vec![5.0, 6.0],
            marker: PhantomData,
            width: 7.0,
        }
    }

    #[test]
    fn map_params_maps_the_fields_in_their_order() {
        let mut slots = Vec::new();
        let mapped = example()
            .map_params(&mut |slot: &str, value: &f64| -> Result<i64, ()> {
                slots.push(slot.to_owned());
                Ok(*value as i64 * 10)
            })
            .unwrap();
        assert_eq!(
            slots,
            ["length", "angle", "inner.x", "pair.0", "pair.1", "width"]
        );
        assert_eq!(
            mapped,
            Example {
                name: "a".to_owned(),
                count: 3,
                length: 10,
                angle: Some(20),
                offset: None,
                inner: Inner { x: 40 },
                pair: vec![50, 60],
                marker: PhantomData,
                width: 70,
            }
        );
    }

    #[test]
    fn map_params_stops_at_the_first_error() {
        let mut slots = Vec::new();
        let result = example().map_params(&mut |slot: &str, _: &f64| -> Result<f64, String> {
            slots.push(slot.to_owned());
            if slot == "inner.x" {
                Err(format!("{slot} refused"))
            } else {
                Ok(0.0)
            }
        });
        assert_eq!(result, Err("inner.x refused".to_owned()));
        assert_eq!(slots, ["length", "angle", "inner.x"]);
    }

    #[test]
    fn map_params_of_a_definition_without_values_calls_nothing() {
        let def: NoValues<f64> = NoValues {
            bodies: vec![1, 2],
            values: PhantomData,
        };
        let mapped = def
            .map_params(&mut |slot: &str, _: &f64| -> Result<u8, ()> { panic!("{slot} mapped") })
            .unwrap();
        assert_eq!(
            mapped,
            NoValues {
                bodies: vec![1, 2],
                values: PhantomData,
            }
        );
    }
}
