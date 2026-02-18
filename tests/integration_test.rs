use sixcy::io_stream::{SixCyWriter, SixCyReader};
use sixcy::CodecId;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tempfile::NamedTempFile;

#[test]
fn test_pack_and_list() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path().to_path_buf();

    let test_data = b"Hello, .6cy format!";
    let file_name = "test.txt".to_string();

    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        writer.add_file(file_name.clone(), test_data, CodecId::Zstd).unwrap();
        writer.finalize().unwrap();
    }

    {
        let mut file = File::open(&archive_path).unwrap();
        let sb = sixcy::superblock::Superblock::read(&mut file).unwrap();
        file.seek(SeekFrom::Start(sb.index_offset)).unwrap();
        let mut index_bytes = vec![0u8; sb.index_size as usize];
        file.read_exact(&mut index_bytes).unwrap();
        let index = sixcy::index::FileIndex::from_bytes(&index_bytes).unwrap();

        assert_eq!(index.records.len(), 1);
        assert_eq!(index.records[0].name, file_name);
        assert_eq!(index.records[0].original_size, test_data.len() as u64);
        assert_eq!(index.records[0].block_refs.len(), 1);
    }
}

#[test]
fn test_pack_unpack_roundtrip() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path().to_path_buf();

    let test_data = b"Roundtrip test data for .6cy v0.1.1";
    let file_name = "roundtrip.txt".to_string();

    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        writer.add_file(file_name.clone(), test_data, CodecId::Zstd).unwrap();
        writer.finalize().unwrap();
    }

    {
        let file = File::open(&archive_path).unwrap();
        let mut reader = SixCyReader::new(file).unwrap();
        let record = reader.index.records[0].clone();
        let unpacked = reader.unpack_file(record.id).unwrap();
        assert_eq!(unpacked, test_data);
    }
}

#[test]
fn test_multifile_pack() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path().to_path_buf();

    let files: Vec<(&str, &[u8])> = vec![
        ("alpha.txt", b"Alpha file contents"),
        ("beta.bin",  b"Beta file contents with different data"),
        ("gamma.txt", b"Gamma file contents here"),
    ];

    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        for (name, data) in &files {
            writer.add_file(name.to_string(), data, CodecId::Zstd).unwrap();
        }
        writer.finalize().unwrap();
    }

    {
        let file = File::open(&archive_path).unwrap();
        let mut reader = SixCyReader::new(file).unwrap();
        assert_eq!(reader.index.records.len(), 3);
        for (i, (name, data)) in files.iter().enumerate() {
            assert_eq!(reader.index.records[i].name, *name);
            let unpacked = reader.unpack_file(reader.index.records[i].id).unwrap();
            assert_eq!(unpacked, *data);
        }
    }
}

#[test]
fn test_cas_deduplication() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path().to_path_buf();

    let shared_data = b"Identical content that should be deduplicated by the CAS engine";

    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        writer.add_file("copy_a.txt".to_string(), shared_data, CodecId::Zstd).unwrap();
        writer.add_file("copy_b.txt".to_string(), shared_data, CodecId::Zstd).unwrap();
        writer.finalize().unwrap();
    }

    {
        let file = File::open(&archive_path).unwrap();
        let mut reader = SixCyReader::new(file).unwrap();
        assert_eq!(reader.index.records.len(), 2);

        // Both records reference the same block offset (CAS dedup)
        let offset_a = reader.index.records[0].block_refs[0].offset;
        let offset_b = reader.index.records[1].block_refs[0].offset;
        assert_eq!(offset_a, offset_b, "CAS deduplication should produce identical offsets");

        // Both should unpack correctly
        let data_a = reader.unpack_file(0).unwrap();
        let data_b = reader.unpack_file(1).unwrap();
        assert_eq!(data_a, shared_data.to_vec());
        assert_eq!(data_b, shared_data.to_vec());
    }
}

#[test]
fn test_lz4_codec() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path().to_path_buf();

    let test_data = b"LZ4 codec roundtrip test";

    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        writer.add_file("lz4_test.bin".to_string(), test_data, CodecId::Lz4).unwrap();
        writer.finalize().unwrap();
    }

    {
        let file = File::open(&archive_path).unwrap();
        let mut reader = SixCyReader::new(file).unwrap();
        let unpacked = reader.unpack_file(0).unwrap();
        assert_eq!(unpacked, test_data);
    }
}

#[test]
fn test_root_hash_is_deterministic() {
    let make_archive = |path: &std::path::Path| {
        let file = File::create(path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        writer.add_file("file.txt".to_string(), b"deterministic", CodecId::Zstd).unwrap();
        writer.finalize().unwrap();
    };

    let t1 = NamedTempFile::new().unwrap();
    let t2 = NamedTempFile::new().unwrap();
    make_archive(t1.path());
    make_archive(t2.path());

    let read_root_hash = |path: &std::path::Path| -> [u8; 32] {
        let mut file = File::open(path).unwrap();
        let sb = sixcy::superblock::Superblock::read(&mut file).unwrap();
        file.seek(SeekFrom::Start(sb.index_offset)).unwrap();
        let mut index_bytes = vec![0u8; sb.index_size as usize];
        file.read_exact(&mut index_bytes).unwrap();
        sixcy::index::FileIndex::from_bytes(&index_bytes).unwrap().root_hash
    };

    assert_eq!(read_root_hash(t1.path()), read_root_hash(t2.path()));
}

#[test]
fn test_superblock_required_codecs() {
    let temp_file = NamedTempFile::new().unwrap();
    let archive_path = temp_file.path().to_path_buf();

    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = SixCyWriter::new(file).unwrap();
        writer.add_file("a.txt".to_string(), b"data", CodecId::Zstd).unwrap();
        writer.add_file("b.bin".to_string(), b"more data", CodecId::Lz4).unwrap();
        writer.finalize().unwrap();
    }

    {
        let mut file = File::open(&archive_path).unwrap();
        let sb = sixcy::superblock::Superblock::read(&mut file).unwrap();
        assert!(sb.required_codecs.contains(&(CodecId::Zstd as u16)));
        assert!(sb.required_codecs.contains(&(CodecId::Lz4 as u16)));
    }
}
