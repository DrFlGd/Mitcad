// SPDX-License-Identifier: MIT
//! The file's own serialisation schema, `Formats/Latest`: every class the
//! file stores, with its base class, version and fields, each field with a
//! type encoding. Class tags are not stored: a class's tag is its
//! definition ordinal, from [`FIRST_TAG`] on, inline definitions counted
//! where they appear. Records name their class by that tag, so the schema
//! is read from each file and no tag is ever assumed.
//!
//! The grammar (see the README): records separated by a zero `u16`; a
//! class is a `u16`-counted ASCII name, a `u16` base reference, a `u32`
//! version, a `u32` field count, the fields (a `u32`-counted name and a
//! type) and a `u32`-counted list of 16-byte GUIDs. A class reference is a
//! `u16`; with bit 15 set the class is defined inline right after a zero
//! `u16`, and the reference's low 15 bits repeat its tag (checked). A type
//! is a `u32` word: kind byte, modifier byte and flags; modifier 0x10 adds
//! a `u32` array count, kind 0x0d adds `01 00 00 00 20` and an inner type
//! (and a class reference when the inner type has one), kind 0x0e with a
//! by-value modifier adds a class reference.

use std::collections::HashMap;
use std::fmt;

use serde::Serialize;

/// The tag of the first class defined.
pub const FIRST_TAG: u16 = 12;

/// Inline definitions nest; real schemas stay a few levels deep.
const MAX_NESTING: usize = 64;

#[derive(Debug, Clone, Serialize)]
pub struct Class {
    pub tag: u16,
    pub name: String,
    /// The tag of the class it derives from.
    pub base: Option<u16>,
    pub version: u32,
    pub fields: Vec<Field>,
    pub guids: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// A field's type as the schema writes it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Type {
    pub word: u32,
    /// The element count of a fixed array (modifier 0x10).
    pub count: Option<u32>,
    /// The element type of kind 0x0d.
    pub inner: Option<Box<Type>>,
    /// The class of a by-value object (kind 0x0e), or of kind 0x0d's
    /// elements.
    pub class: Option<u16>,
}

impl Type {
    pub fn kind(&self) -> u8 {
        (self.word & 0xff) as u8
    }

    pub fn modifier(&self) -> u8 {
        ((self.word >> 8) & 0xff) as u8
    }

    /// The word's high half: flags (see [`FLAG_NOT_STORED`]).
    pub fn flags(&self) -> u16 {
        (self.word >> 16) as u16
    }
}

/// The type flag of fields that are not stored (seen only in the 2027
/// schema, on fields named as temporary, cached or unsaved state; walks of
/// every 2027 corpus object agree).
pub const FLAG_NOT_STORED: u16 = 0x02;

/// How a field's value is stored, as the walker reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Scalar {
    /// Kind 0x01: one byte.
    Bool,
    /// Kind 0x02: one byte.
    U8,
    /// Kind 0x03: two bytes.
    U16,
    /// Kinds 0x04 and 0x05: four bytes.
    U32,
    /// Kind 0x06.
    F32,
    /// Kind 0x07.
    F64,
    /// Kind 0x08 (modifier 0x60): a `u32` count of UTF-16 units, then the
    /// units.
    Text,
    /// Kind 0x09: 16 bytes.
    Guid,
    /// Kind 0x0a: a class tag, two bytes.
    ClassTag,
    /// Kind 0x0b: eight bytes.
    U64,
}

/// The pointer forms of kind 0x0e (the modifier's low nibble).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PointerKind {
    /// 1: an owned object: a `u32` (0 null; all ones or an object number)
    /// and a `u16` class tag; the object follows the record's fields.
    Owned,
    /// 2: stored as [`PointerKind::Owned`] (what sets it apart is not
    /// known).
    Value,
    /// 3: a `u32` (the document, or an object written elsewhere).
    Reference,
    /// 4: stored as [`PointerKind::Owned`] (meaning not known).
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Encoding {
    Scalar(Scalar),
    /// An object of the class, its fields in place.
    Object(u16),
    Pointer(PointerKind),
    /// A fixed number of elements.
    Array(u32, Box<Encoding>),
    /// A `u32` count, then the elements.
    Container(Box<Encoding>),
}

/// A type word outside the grammar the walker knows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnknownEncoding {
    pub class: String,
    pub field: String,
    pub word: u32,
    pub why: String,
}

impl fmt::Display for UnknownEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}: type word {:#x}: {}",
            self.class, self.field, self.word, self.why
        )
    }
}

/// The type encodings whose storage is known but whose meaning is not:
/// listed in the README and in `mitcad-rvt-inspect schema`.
pub const KNOWN_UNKNOWNS: [(&str, &str); 6] = [
    (
        "flag 0x04",
        "type words with 0x04 in the flags (VarExpr.m_refCt in every release): stored as the kind says; meaning not known",
    ),
    (
        "flags 0x10, 0x20",
        "seen only together with the not-stored flag 0x02 (2027): not stored; meaning not known",
    ),
    (
        "pointer kind 2",
        "stored as an owned pointer (u32, class tag, the object after the record's fields); what sets it apart is not known",
    ),
    (
        "pointer kind 4",
        "stored as an owned pointer; meaning not known (variational sketch objects)",
    ),
    (
        "kinds 0x04 and 0x05",
        "both four-byte integers; which is signed is not known",
    ),
    (
        "class GUID lists",
        "each class ends with a counted list of 16-byte GUIDs; their meaning is not known",
    ),
];

impl Type {
    /// How the walker reads it, or why it cannot.
    pub fn encoding(&self) -> Result<Encoding, String> {
        let flags = self.flags();
        if flags & !(0x02 | 0x04 | 0x10 | 0x20) != 0 {
            return Err(format!("flags {flags:#06x}"));
        }
        let (kind, modifier) = (self.kind(), self.modifier());
        let (form, low) = (modifier & 0xf0, modifier & 0x0f);
        let element = match kind {
            0x0d => self
                .inner
                .as_ref()
                .ok_or("kind 0x0d without an inner type")?
                .encoding()?,
            0x0e => match low {
                0 => Encoding::Object(self.class.ok_or("an object type without a class")?),
                1 => Encoding::Pointer(PointerKind::Owned),
                2 => Encoding::Pointer(PointerKind::Value),
                3 => Encoding::Pointer(PointerKind::Reference),
                4 => Encoding::Pointer(PointerKind::Other),
                _ => return Err(format!("pointer form {low}")),
            },
            0x08 if modifier == 0x60 => return Ok(Encoding::Scalar(Scalar::Text)),
            _ => {
                if low != 0 {
                    return Err(format!("modifier {modifier:#04x} on kind {kind:#04x}"));
                }
                Encoding::Scalar(match kind {
                    0x01 => Scalar::Bool,
                    0x02 => Scalar::U8,
                    0x03 => Scalar::U16,
                    0x04 | 0x05 => Scalar::U32,
                    0x06 => Scalar::F32,
                    0x07 => Scalar::F64,
                    0x09 => Scalar::Guid,
                    0x0a => Scalar::ClassTag,
                    0x0b => Scalar::U64,
                    _ => return Err(format!("kind {kind:#04x}")),
                })
            }
        };
        match form {
            0x00 => Ok(element),
            0x10 => Ok(Encoding::Array(
                self.count.ok_or("an array without a count")?,
                Box::new(element),
            )),
            0x50 => Ok(Encoding::Container(Box::new(element))),
            _ => Err(format!("modifier {modifier:#04x}")),
        }
    }

    /// Whether the field's value is stored at all.
    pub fn stored(&self) -> bool {
        self.flags() & FLAG_NOT_STORED == 0
    }
}

/// The parsed schema.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Schema {
    /// By tag: `classes[tag - FIRST_TAG]`.
    pub classes: Vec<Class>,
    #[serde(skip)]
    names: HashMap<String, u16>,
    /// The ElementId class's tag (its objects are read as ids).
    pub element_id: u16,
    /// Bytes of an ElementId: 4 through the 2023 release, 8 from 2024.
    pub id_width: usize,
    /// Bytes of the schema read (all of it unless `stopped`).
    pub parsed: usize,
    pub size: usize,
    /// Why the reading stopped before the end.
    pub stopped: Option<String>,
}

impl Schema {
    pub fn class(&self, tag: u16) -> Option<&Class> {
        self.classes.get(usize::from(tag.checked_sub(FIRST_TAG)?))
    }

    pub fn class_name(&self, tag: u16) -> &str {
        self.class(tag).map_or("?", |c| c.name.as_str())
    }

    pub fn tag(&self, name: &str) -> Option<u16> {
        self.names.get(name).copied()
    }

    /// The class and its bases, the root base first.
    pub fn lineage(&self, tag: u16) -> Vec<u16> {
        let mut out = Vec::new();
        let mut at = Some(tag);
        while let Some(t) = at {
            if out.contains(&t) || out.len() > MAX_NESTING {
                break;
            }
            out.push(t);
            at = self.class(t).and_then(|c| c.base);
        }
        out.reverse();
        out
    }

    /// Whether `tag` is the class `name` or derives from it.
    pub fn is_a(&self, tag: u16, name: &str) -> bool {
        self.tag(name)
            .is_some_and(|base| self.lineage(tag).contains(&base))
    }

    /// Every field type the walker cannot read (none on the corpus).
    pub fn unknown_encodings(&self) -> Vec<UnknownEncoding> {
        let mut out = Vec::new();
        for class in &self.classes {
            for field in &class.fields {
                if let Err(why) = field.ty.encoding() {
                    out.push(UnknownEncoding {
                        class: class.name.clone(),
                        field: field.name.clone(),
                        word: field.ty.word,
                        why,
                    });
                }
            }
        }
        out
    }

    /// Every type word with how many fields use it.
    pub fn words(&self) -> Vec<(u32, usize)> {
        let mut counts: HashMap<u32, usize> = HashMap::new();
        fn visit(ty: &Type, counts: &mut HashMap<u32, usize>) {
            *counts.entry(ty.word).or_default() += 1;
            if let Some(inner) = &ty.inner {
                visit(inner, counts);
            }
        }
        for class in &self.classes {
            for field in &class.fields {
                visit(&field.ty, &mut counts);
            }
        }
        let mut out: Vec<_> = counts.into_iter().collect();
        out.sort();
        out
    }

    /// Reads a schema (the inflated `Formats/Latest`).
    pub fn parse(data: &[u8]) -> Result<Schema, String> {
        let mut reader = Reader {
            data,
            next_tag: FIRST_TAG,
            classes: Vec::new(),
        };
        let mut at = 0;
        let stopped = loop {
            if at + 2 > data.len() {
                break None;
            }
            if reader.u16(at) != Some(0) {
                break Some(format!("record separator is not 0 at {at:#x}"));
            }
            at += 2;
            if at >= data.len() || data[at..].iter().all(|&b| b == 0) {
                at = data.len();
                break None;
            }
            match reader.class(at, 0) {
                Ok((_, next)) => at = next,
                Err(e) => break Some(e),
            }
        };
        let mut classes = reader.classes;
        classes.sort_by_key(|c| c.tag);
        if classes.is_empty() {
            return Err(stopped.unwrap_or_else(|| "the schema has no classes".into()));
        }
        // Tags are dense from FIRST_TAG (each definition takes the next).
        for (i, c) in classes.iter().enumerate() {
            if usize::from(c.tag) != usize::from(FIRST_TAG) + i {
                return Err(format!("class tags are not dense at {}", c.name));
            }
        }
        let names = classes.iter().map(|c| (c.name.clone(), c.tag)).collect();
        let mut schema = Schema {
            classes,
            names,
            element_id: 0,
            id_width: 0,
            parsed: at,
            size: data.len(),
            stopped,
        };
        schema.element_id = schema
            .tag("ElementId")
            .ok_or("the schema has no ElementId class")?;
        // ElementId holds an Identifier, whose one field is the number.
        let identifier = schema
            .tag("Identifier")
            .and_then(|t| schema.class(t))
            .and_then(|c| c.fields.first())
            .ok_or("the schema has no Identifier class")?;
        schema.id_width = match identifier.ty.encoding() {
            Ok(Encoding::Scalar(Scalar::U32)) => 4,
            Ok(Encoding::Scalar(Scalar::U64)) => 8,
            other => return Err(format!("an Identifier of {other:?}")),
        };
        Ok(schema)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    next_tag: u16,
    classes: Vec<Class>,
}

impl Reader<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            self.data.get(at..at + 2)?.try_into().ok()?,
        ))
    }

    fn u32(&self, at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.data.get(at..at + 4)?.try_into().ok()?,
        ))
    }

    fn need<T>(value: Option<T>, what: &str, at: usize) -> Result<T, String> {
        value.ok_or_else(|| format!("{what} past the end at {at:#x}"))
    }

    fn name(&self, at: usize, width: usize) -> Result<(String, usize), String> {
        let len = if width == 2 {
            usize::from(Self::need(self.u16(at), "name length", at)?)
        } else {
            Self::need(self.u32(at), "name length", at)? as usize
        };
        let start = at + width;
        let raw = (1..=250)
            .contains(&len)
            .then(|| self.data.get(start..start + len))
            .flatten()
            .ok_or_else(|| format!("bad name length {len} at {at:#x}"))?;
        if !raw.iter().all(|b| (0x20..0x7f).contains(b)) {
            return Err(format!("name is not ASCII at {at:#x}"));
        }
        Ok((String::from_utf8_lossy(raw).into_owned(), start + len))
    }

    fn inline_class(
        &mut self,
        at: usize,
        reference: u16,
        depth: usize,
    ) -> Result<(u16, usize), String> {
        if self.u16(at) != Some(0) {
            return Err(format!("inline pad is not 0 at {at:#x}"));
        }
        let (tag, next) = self.class(at + 2, depth + 1)?;
        if tag != reference & 0x7fff {
            return Err(format!(
                "inline tag {tag} does not match reference {} at {at:#x}",
                reference & 0x7fff
            ));
        }
        Ok((tag, next))
    }

    fn class_ref(&mut self, at: usize, depth: usize) -> Result<(u16, usize), String> {
        let reference = Self::need(self.u16(at), "class reference", at)?;
        if reference & 0x8000 == 0 {
            return Ok((reference, at + 2));
        }
        self.inline_class(at + 2, reference, depth)
    }

    fn field_type(&mut self, at: usize, depth: usize) -> Result<(Type, usize), String> {
        if depth > MAX_NESTING {
            return Err(format!("types nest too deep at {at:#x}"));
        }
        let word = Self::need(self.u32(at), "type word", at)?;
        let mut ty = Type {
            word,
            count: None,
            inner: None,
            class: None,
        };
        let mut at = at + 4;
        let (kind, modifier) = (ty.kind(), ty.modifier());
        if !matches!(modifier & 0xf0, 0x00 | 0x10 | 0x50 | 0x60) {
            return Err(format!("unknown modifier {modifier:#04x} at {:#x}", at - 4));
        }
        if modifier & 0xf0 == 0x10 {
            let count = Self::need(self.u32(at), "array count", at)?;
            if count > 65_536 {
                return Err(format!("array count {count} at {at:#x}"));
            }
            ty.count = Some(count);
            at += 4;
        }
        if kind == 0x0d {
            if self.data.get(at..at + 5) != Some(&[1, 0, 0, 0, 0x20][..]) {
                return Err(format!("array filler at {at:#x}"));
            }
            let (inner, next) = self.field_type(at + 5, depth + 1)?;
            at = next;
            if inner.class.is_some() {
                let (class, next) = self.class_ref(at, depth)?;
                ty.class = Some(class);
                at = next;
            }
            ty.inner = Some(Box::new(inner));
            return Ok((ty, at));
        }
        if kind == 0x0e && modifier & 0x0f == 0 {
            let (class, next) = self.class_ref(at, depth)?;
            ty.class = Some(class);
            at = next;
        }
        Ok((ty, at))
    }

    fn class(&mut self, at: usize, depth: usize) -> Result<(u16, usize), String> {
        if depth > MAX_NESTING {
            return Err(format!("classes nest too deep at {at:#x}"));
        }
        let (name, mut p) = self.name(at, 2)?;
        let tag = self.next_tag;
        self.next_tag = tag
            .checked_add(1)
            .filter(|next| *next < 0x8000)
            .ok_or_else(|| format!("too many classes at {at:#x}"))?;
        let reference = Self::need(self.u16(p), "base reference", p)?;
        p += 2;
        let base = if reference & 0x8000 != 0 {
            let (base, next) = self.inline_class(p, reference, depth)?;
            p = next;
            Some(base)
        } else {
            (reference != 0).then_some(reference)
        };
        let version = Self::need(self.u32(p), "version", p)?;
        let field_count = Self::need(self.u32(p + 4), "field count", p + 4)?;
        p += 8;
        if field_count > 2_000 {
            return Err(format!("field count {field_count} at {:#x}", p - 4));
        }
        let mut fields = Vec::with_capacity(field_count as usize);
        for _ in 0..field_count {
            let (name, next) = self.name(p, 4)?;
            let (ty, next) = self.field_type(next, depth)?;
            fields.push(Field { name, ty });
            p = next;
        }
        let guids = Self::need(self.u32(p), "GUID count", p)? as usize;
        p += 4;
        p = guids
            .checked_mul(16)
            .and_then(|len| p.checked_add(len))
            .filter(|end| *end <= self.data.len() && guids <= 60_000)
            .ok_or_else(|| format!("GUID count {guids} at {:#x}", p - 4))?;
        self.classes.push(Class {
            tag,
            name,
            base,
            version,
            fields,
            guids,
        });
        Ok((tag, p))
    }
}

/// Writes a schema in the file's grammar (for test files): classes in
/// order, their tags from [`FIRST_TAG`]; bases and class references by
/// tag (no inline definitions).
#[derive(Default)]
pub struct SchemaWriter {
    out: Vec<u8>,
    next: u16,
}

/// A field for [`SchemaWriter::class`]: name and type.
pub type FieldSpec<'a> = (&'a str, Type);

impl SchemaWriter {
    pub fn new() -> Self {
        SchemaWriter {
            out: Vec::new(),
            next: FIRST_TAG,
        }
    }

    /// A class; returns its tag.
    pub fn class(&mut self, name: &str, base: Option<u16>, fields: &[FieldSpec<'_>]) -> u16 {
        self.out.extend(0u16.to_le_bytes());
        self.out.extend((name.len() as u16).to_le_bytes());
        self.out.extend(name.as_bytes());
        self.out.extend(base.unwrap_or(0).to_le_bytes());
        self.out.extend(1u32.to_le_bytes());
        self.out.extend((fields.len() as u32).to_le_bytes());
        for (field, ty) in fields {
            self.out.extend((field.len() as u32).to_le_bytes());
            self.out.extend(field.as_bytes());
            write_type(&mut self.out, ty);
        }
        self.out.extend(0u32.to_le_bytes());
        let tag = self.next;
        self.next += 1;
        tag
    }

    /// The tag the next class gets.
    pub fn next_tag(&self) -> u16 {
        self.next
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.out.extend(0u16.to_le_bytes());
        self.out
    }
}

fn write_type(out: &mut Vec<u8>, ty: &Type) {
    out.extend(ty.word.to_le_bytes());
    if let Some(count) = ty.count {
        out.extend(count.to_le_bytes());
    }
    if let Some(inner) = &ty.inner {
        out.extend([1, 0, 0, 0, 0x20]);
        write_type(out, inner);
        if inner.class.is_some() {
            out.extend(ty.class.unwrap_or(0).to_le_bytes());
        }
        return;
    }
    if let Some(class) = ty.class {
        out.extend(class.to_le_bytes());
    }
}

/// Types for [`SchemaWriter`].
pub mod types {
    use super::Type;

    pub fn word(word: u32) -> Type {
        Type {
            word,
            count: None,
            inner: None,
            class: None,
        }
    }
    pub fn object(class: u16) -> Type {
        Type {
            class: Some(class),
            ..word(0x0e)
        }
    }
    pub fn container_of(class: u16) -> Type {
        Type {
            class: Some(class),
            ..word(0x500e)
        }
    }
    pub fn array(kind: u32, count: u32) -> Type {
        Type {
            count: Some(count),
            ..word(0x1000 | kind)
        }
    }
    /// An array of `outer` arrays of `inner` doubles.
    pub fn matrix(outer: u32, inner: u32) -> Type {
        Type {
            count: Some(outer),
            inner: Some(Box::new(array(0x07, inner))),
            ..word(0x100d)
        }
    }
    pub const BOOL: u32 = 0x01;
    pub const U32: u32 = 0x04;
    pub const F64: u32 = 0x07;
    pub const U64: u32 = 0x0b;
    pub const TEXT: u32 = 0x6008;
    pub const CLASS_TAG: u32 = 0x0a;
    pub const OWNED: u32 = 0x010e;
    pub const DOC: u32 = 0x030e;
}

#[cfg(test)]
mod tests {
    use super::types::*;
    use super::*;

    #[test]
    fn written_schema_reads_back() {
        let mut w = SchemaWriter::new();
        let identifier = w.class("Identifier", None, &[("m_id64", word(U64))]);
        let id = w.class("ElementId", None, &[("m_id", object(identifier))]);
        let base = w.class("Base", None, &[("m_flag", word(BOOL))]);
        let derived = w.class(
            "Derived",
            Some(base),
            &[
                ("m_name", word(TEXT)),
                ("m_ids", container_of(id)),
                ("m_trf", matrix(4, 3)),
                ("m_weird", word(0x7f)),
            ],
        );
        let schema = Schema::parse(&w.finish()).unwrap();
        assert_eq!(schema.classes.len(), 4);
        assert_eq!(schema.id_width, 8);
        assert_eq!(schema.element_id, id);
        assert_eq!(schema.class_name(derived), "Derived");
        assert_eq!(schema.lineage(derived), vec![base, derived]);
        assert!(schema.is_a(derived, "Base"));
        let fields = &schema.class(derived).unwrap().fields;
        assert_eq!(
            fields[1].ty.encoding(),
            Ok(Encoding::Container(Box::new(Encoding::Object(id))))
        );
        assert_eq!(
            fields[2].ty.encoding(),
            Ok(Encoding::Array(
                4,
                Box::new(Encoding::Array(3, Box::new(Encoding::Scalar(Scalar::F64))))
            ))
        );
        // The unknown kind is reported, by class and field.
        let unknown = schema.unknown_encodings();
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].field, "m_weird");
        assert!(unknown[0].to_string().contains("Derived.m_weird"));
    }

    #[test]
    fn encodings_of_the_corpus_words() {
        for (w, ok) in [
            (0x01, true),
            (0x6008, true),
            (0x010e, true),
            (0x030e, true),
            (0x040e, true),
            (0x510e, true),
            (0x540e, true),
            (0x130e, true),
            (0x5002, true),
            (0x40004, true),
            (0x220001, true),
            (0x050e, false),
            (0x1008, false),
            (0x0c, false),
            (0x80004, false),
            (0x7004, false),
        ] {
            let mut ty = word(w);
            if ty.modifier() & 0xf0 == 0x10 {
                ty.count = Some(2);
            }
            assert_eq!(ty.encoding().is_ok(), ok, "{w:#x}");
        }
        assert!(!word(0x220001).stored());
        assert!(word(0x40004).stored());
    }

    #[test]
    fn damaged_schemas_are_errors() {
        assert!(Schema::parse(&[]).is_err());
        assert!(Schema::parse(&[0, 0, 3, 0, b'A', 0xff, b'C']).is_err());
        let mut w = SchemaWriter::new();
        w.class("Identifier", None, &[("m_id", word(U32))]);
        let mut bytes = w.finish();
        // A field count past the end.
        bytes[2 + 2 + 10 + 2 + 4] = 0xff;
        assert!(Schema::parse(&bytes).is_err());
    }
}
