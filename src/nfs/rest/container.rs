// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.
use nfs;
use routing;
use time;
use client;

pub struct Container {
    client: ::std::sync::Arc<::std::sync::Mutex<client::Client>>,
    directory_listing: nfs::directory_listing::DirectoryListing
}

impl Container {
    /// Authorizes the root directory access and return the Container
    /// Entry point for the Rest API
    pub fn authorise(client: ::std::sync::Arc<::std::sync::Mutex<client::Client>>, dir_id: [u8;64]) -> Result<Container, String> {
        let mut directory_helper = nfs::helper::DirectoryHelper::new(client.clone());
        let result = directory_helper.get(&::routing::NameType(dir_id));
        match result {
            Ok(listing) => Ok(Container {
                client: client,
                directory_listing: listing
            }),
            Err(msg) => Err(msg)
        }
    }

    pub fn get_id(&self) -> [u8;64] {
        self.directory_listing.get_id().0
    }

    pub fn get_metadata(&self) -> Option<String> {
        let metadata = self.directory_listing.get_metadata().get_user_metadata();
        match metadata {
            Some(data) => Some(String::from_utf8(data.clone()).unwrap()),
            None => None
        }
    }

    pub fn get_name(&self) -> &String {
        self.directory_listing.get_metadata().get_name()
    }

    pub fn get_created_time(&self) -> time::Tm {
        self.directory_listing.get_metadata().get_created_time()
    }

    pub fn get_modified_time(&self) -> time::Tm {
        self.directory_listing.get_metadata().get_modified_time()
    }

    pub fn get_blobs(&self) -> Vec<nfs::rest::Blob> {
        self.directory_listing.get_files().iter().map(|x| nfs::rest::Blob::convert_from_file(self.client.clone(), x.clone())).collect()
    }

    pub fn get_blob(&self, name: String, version: Option<[u8;64]>) -> Result<nfs::rest::Blob, String> {
        match version {
            Some(version_id) => {
                let dir_id = self.directory_listing.get_id();
                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                match directory_helper.get_by_version(dir_id, &routing::NameType(version_id)) {
                    Ok(listing) => match self.find_file(&name, &listing){
                        Some(blob) => Ok(blob),
                        None => Err("Blob not found for the version specified".to_string())
                    },
                    Err(msg) => Err(msg)
                }
            },
            None => match self.find_file(&name, &self.directory_listing) {
                Some(blob) => Ok(blob),
                None => Err("Blob not found for the version specified".to_string())
            },
        }
    }

    pub fn create(&mut self, name: &String, metadata: Option<String>) -> Result<(), String> {
        match self.validate_metadata(metadata) {
            Ok(user_metadata) => {
                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                match directory_helper.create(name.clone(), user_metadata) {
                    Ok(dir_id) => {
                        let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                        match directory_helper.get(&dir_id) {
                            Ok(created_directory) => {
                                self.directory_listing.get_mut_sub_directories().push(created_directory.get_info().clone());
                                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                                match directory_helper.update(&created_directory) {
                                    Ok(_) => Ok(()),
                                    Err(msg) => Err(msg)
                                }
                            },
                            Err(msg) => Err(msg)
                        }
                    },
                    Err(msg) => Err(msg)
                }
            },
            Err(err) => Err(err),
        }
    }

    pub fn get_containers(&self) -> Vec<nfs::rest::ContainerInfo> {
        self.directory_listing.get_sub_directories().iter().map(|info| {
                nfs::rest::ContainerInfo::convert_from_directory_info(info.clone())
            }).collect()
    }

    pub fn update_metadata(&mut self, metadata: Option<String>) -> Result<(), String>{
        match self.validate_metadata(metadata) {
            Ok(user_metadata) => {
                self.directory_listing.set_user_metadata(user_metadata);
                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                match directory_helper.update(&self.directory_listing) {
                    Ok(_) => Ok(()),
                    Err(msg) => Err(msg),
                }
            },
            Err(err) => Err(err),
        }
    }

    pub fn get_container(&mut self, name: &String, version: Option<[u8; 64]>) -> Result<Container, String> {
        let sub_dirs = self.directory_listing.get_sub_directories();
        match sub_dirs.iter().find(|&entry| *entry.get_name() == *name) {
            Some(dir_info) => {
                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                let get_dir_listing_result = match version {
                    Some(version_id) => directory_helper.get_by_version(dir_info.get_id(), &::routing::NameType(version_id)),
                    None =>  directory_helper.get(dir_info.get_id())
                };
                match get_dir_listing_result {
                    Ok(dir_listing) => Ok(Container::convert_from_directory_listing(self.client.clone(), dir_listing)),
                    Err(msg) => Err(msg)
                }
            },
            None => Err("Container not found".to_string())
        }
    }

    pub fn get_versions(&mut self) -> Result<Vec<[u8;64]>, String> {
        let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
        match directory_helper.get_versions(self.directory_listing.get_id()) {
            Ok(versions) => {
                Ok(versions.iter().map(|v| v.0).collect())
            },
            Err(msg) => Err(msg)
        }
    }

    pub fn delete_container(&mut self, name: &String) -> Result<(), String> {
        match self.directory_listing.get_sub_directories().iter().position(|entry| *entry.get_name() == *name) {
            Some(pos) => {
                self.directory_listing.get_mut_sub_directories().remove(pos);
                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                match directory_helper.update(&self.directory_listing) {
                    Ok(_) => Ok(()),
                    Err(msg) => Err(msg)
                }
            },
            None => {
                Err("Container not found".to_string())
            }
        }
    }

    pub fn create_blob(&mut self, name: String, metadata: Option<String>, size: u64) -> Result<nfs::io::Writer, String> {
        match self.validate_metadata(metadata) {
            Ok(user_metadata) => {
                let mut file_helper = nfs::helper::FileHelper::new(self.client.clone());
                file_helper.create(name, size, user_metadata, &self.directory_listing)
            },
            Err(err) => Err(err),
        }
    }
        
    pub fn update_blob_content(&mut self, blob: &nfs::rest::Blob, data: &[u8]) -> Result<(), String> {
        match self.get_writer_for_blob(blob) {
            Ok(mut writer) => {
                writer.write(data, 0);
                Ok(())
            },
            Err(err) => Err(err),
        }
    }

    pub fn get_blob_writer(&mut self, blob: &nfs::rest::Blob, data: Vec<u8>) -> Result<nfs::io::Writer, String> {
        self.get_writer_for_blob(blob)
    }

    pub fn get_blob_content(&mut self, blob: nfs::rest::Blob) -> Result<Vec<u8>, String> {
        match self.get_reader_for_blob(blob) {
            Ok(mut reader) => {
                let size = reader.size();
                reader.read(0, size)
            },
            Err(msg) => Err(msg)
        }
    }

    pub fn get_blob_reader(&mut self, blob: nfs::rest::blob::Blob) -> Result<nfs::io::reader::Reader, String> {
        self.get_reader_for_blob(blob)
    }

    pub fn get_blob_versions(&mut self, name: &String) -> Result<Vec<[u8;64]>, String>{
        match self.find_file(name, &self.directory_listing) {
            Some(blob) => {
                let mut file_helper = nfs::helper::FileHelper::new(self.client.clone());
                let versions = file_helper.get_versions(self.directory_listing.get_id(), &blob.convert_to_file());
                Ok(Vec::new())
            },
            None=> Err("Blob not found".to_string())
        }
    }

    pub fn delete_blob(&mut self, name: &String) -> Result<(), String> {
        match self.directory_listing.get_sub_directories().iter().position(|file| *file.get_name() == *name) {
            Some(pos) => {
                self.directory_listing.get_mut_sub_directories().remove(pos);
                let mut directory_helper = nfs::helper::DirectoryHelper::new(self.client.clone());
                match directory_helper.update(&self.directory_listing) {
                    Ok(_) => Ok(()),
                    Err(msg) => Err(msg)
                }
            },
            None => Err("Blob not found".to_string())
        }
    }

    fn get_writer_for_blob(&self, blob: &nfs::rest::blob::Blob) -> Result<nfs::io::Writer, String> {
        let mut helper = nfs::helper::FileHelper::new(self.client.clone());
        match helper.update(blob.convert_to_file(), &self.directory_listing) {
            Ok(writter) => Ok(writter),
            Err(_) => Err("Blob not found".to_string())
        }
    }

    fn get_reader_for_blob(&self, blob: nfs::rest::blob::Blob) -> Result<nfs::io::Reader, String> {
        match self.find_file(blob.get_name(), &self.directory_listing) {
            Some(_) => {
                Ok(nfs::io::Reader::new(blob.convert_to_file(), self.client.clone()))
            },
            None => Err("Blob not found".to_string())
        }
    }

    fn validate_metadata(&self, metadata: Option<String>) -> Result<Vec<u8>, String> {
        match metadata {
            Some(data) => {
                if data.len() == 0 {
                    Err("Metadata cannot be empty".to_string())
                } else {
                    Ok(data.into_bytes())
                }
            },
            None => Ok(Vec::new()),
        }
    }

    fn find_file(&self, name: &String, directory_listing: &nfs::directory_listing::DirectoryListing) -> Option<nfs::rest::Blob> {
        match directory_listing.get_files().iter().find(|file| file.get_name() == name) {
            Some(file) => Some(nfs::rest::Blob::convert_from_file(self.client.clone(), file.clone())),
            None => None
        }
    }

    pub fn convert_to_directory_listing(&self) -> nfs::directory_listing::DirectoryListing {
        self.directory_listing.clone()
    }

    pub fn convert_from_directory_listing(client: ::std::sync::Arc<::std::sync::Mutex<client::Client>>,
         directory_listing: nfs::directory_listing::DirectoryListing) -> Container {
        Container {
            client: client,
            directory_listing: directory_listing
        }
    }
}
