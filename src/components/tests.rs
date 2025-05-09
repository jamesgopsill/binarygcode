use crate::components::deserialiser::{DeserialisedResult, Deserialiser};

// TODO: Make some more robust tests.
#[test]
fn deser_test_file() {
    let mut deserialiser = Deserialiser::default();
    deserialiser.digest(include_bytes!("../../test_files/mini_cube_b.bgcode"));

    loop {
        let r = deserialiser.deserialise().unwrap();
        match r {
            DeserialisedResult::MoreBytesRequired(_) => {
                break;
            }
            _ => (),
        }
    }
}
