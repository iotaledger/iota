// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use crate::stardust::types::address_swap_map::AddressSwapMap;

    fn write_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_from_csv_valid_file() {
        let content = "Origin,Destination\n\
                       iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);
        assert!(result.is_ok());
        let map = result.unwrap();
        assert_eq!(map.addresses().len(), 1);
    }

    #[test]
    fn test_from_csv_missing_file() {
        let result = AddressSwapMap::from_csv("nonexistent_file.csv");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_csv_invalid_headers() {
        let content = "wrong_header1,wrong_header2\n\
                       iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_csv_invalid_record() {
        let content = "Origin,Destination\n\
                       iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c,invalid_number";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_from_csv_empty_file() {
        let content = "Origin,Destination";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);
        assert!(result.is_ok());
        let map = result.unwrap();

        assert_eq!(map.addresses().len(), 0);
    }
}
