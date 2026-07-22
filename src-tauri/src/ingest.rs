//! The ingest copy job: the only filesystem-touching part of SD ingest. Pure
//! recognition/enumeration/naming/verification live in `katto_engine::ingest`.

pub mod copy;
