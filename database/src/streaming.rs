use serde::de::{DeserializeOwned, SeqAccess, Visitor};
use serde::Deserializer as _;
use std::fmt;
use std::io::Read;
use std::marker::PhantomData;

/// Stream-deserialize a top-level JSON array of `T`, invoking `f` for every
/// element. Unlike `serde_json::from_reader::<_, Vec<T>>` this never builds the
/// full `Vec<T>`, so peak memory stays at one element (plus whatever `f` keeps).
pub fn for_each_array<R, T, F>(reader: R, f: F) -> Result<(), serde_json::Error>
where
    R: Read,
    T: DeserializeOwned,
    F: FnMut(T),
{
    struct EntrySeq<F, T>(F, PhantomData<fn() -> T>);

    impl<'de, T, F> Visitor<'de> for EntrySeq<F, T>
    where
        T: DeserializeOwned,
        F: FnMut(T),
    {
        type Value = ();

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a JSON array")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut f = self.0;
            while let Some(item) = seq.next_element::<T>()? {
                f(item);
            }
            Ok(())
        }
    }

    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    deserializer.deserialize_seq(EntrySeq(f, PhantomData))?;
    Ok(())
}
