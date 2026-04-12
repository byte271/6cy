use sixcy::io_stream::SixCyWriter;
use sixcy::CodecId;
use std::fs::File;
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
        writer
            .add_file(file_name.clone(), test_data, CodecId::Zstd)
            .unwrap();
        writer.finalize().unwrap();
    }

    {
        let file = File::open(&archive_path).unwrap();
        let reader = sixcy::io_stream::SixCyReader::new(file).unwrap();

        let index = &reader.index;
        assert_eq!(index.records.len(), 1);
        assert_eq!(index.records[0].name, file_name);
        assert_eq!(index.records[0].original_size, test_data.len() as u64);
    }
}
