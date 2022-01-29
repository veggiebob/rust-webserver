use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::io::Write;
use std::pin::Pin;
use std::str::FromStr;
use std::task::Poll;
use chrono::format::parse;
use chrono::NaiveDateTime;
use crate::server::Response;
/*

The cache should store requests from the user.

File structure:

cache/
    cache-meta/
        cache-index
    data/
        <hash1>/
            0/
                key
                data
            1/
                key
                data
            ...
        <hash2>/
            0/
                key
                data
            1/
                key
                data
            ...
        ...

 */


// the index should store the requests that have been cached.

struct CacheIndex<'a> {
    filename: &'a str,

    entries: HashMap<String, chrono::NaiveDateTime>
}

struct Cache<'a> {
    folder: &'a str,
    index: CacheIndex<'a>
}

const ENTRY_SPLITTER: &str = "%%%";
const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

impl CacheIndex<'_> {

    pub fn new(filename: &str) -> Result<CacheIndex, String> {
        let file = OpenOptions::new()
            .create(true).write(true) // allow creating, and thus writing
            .read(true) // be able to read file!
            .open(filename);
        let mut entries = HashMap::new();
        match file {
            Ok(file) => {
                for line in BufReader::new(file).lines() {
                    if let Ok(line) = line {
                        if let Some((before, after)) = line.split_once(ENTRY_SPLITTER) {
                            let after = String::from(after);
                            let time = after.trim();
                            if let Ok(time) = NaiveDateTime::parse_from_str(time, TIME_FORMAT) {
                                entries.insert(
                                    String::from(before).trim().into(),
                                    time
                                );
                            }
                        }
                    }
                }
                Ok(CacheIndex {
                    filename,
                    entries
                })
            }
            Err(e) => {
                Err(format!("Could not create CacheIndex from filename '{}'", e))
            }
        }
    }

    pub fn update_file(&self) -> std::io::Result<()> {
        let mut file = File::create(self.filename)?;
        write!(file, "{}", self.entries.iter().fold(String::new(), |str, (name, time)| {
            str + "\n" + &*(name.to_string() + ENTRY_SPLITTER + &*time.format(TIME_FORMAT).to_string())
        }));
        Ok(())
    }

    /// returns an error if the file does not exist
    pub fn clear_cache(&mut self) -> std::io::Result<()> {
        std::fs::remove_file(self.filename).map(|e| {
            self.entries.clear();
            e
        })
    }

    pub fn get_entries(&self) -> &HashMap<String, chrono::NaiveDateTime> {
        &self.entries
    }
}

fn get_sub_folders(folder: &str) -> std::io::Result<HashSet<String>> {
    let dir = std::fs::read_dir(folder)?;
    Ok(dir.into_iter()
        .filter_map(|file| {
            match file {
                Ok(entry) => {
                    entry.metadata().ok().and_then(|meta|
                        if meta.is_dir() {
                            let path = entry.path().as_path().display();
                            Some(format!("{}", entry.file_name().to_str().unwrap()))
                        } else {
                            None
                        }
                    )
                },
                Err(e) => {
                    None
                }
            }
        }).collect())
}

impl Cache<'_> {

    pub fn new<'a>(index_filename: &'a str, cache_folder: &'a str) -> Result<Cache<'a>, String> {
        let cache_index = CacheIndex::new(index_filename)?;
        std::fs::create_dir_all(cache_folder)
            .map_err(|e| e.to_string())?; // create the cache folder, or get it
        Ok(Cache {
            folder: cache_folder,
            index: cache_index
        })
    }

    fn get_sub_folders(&self) -> std::io::Result<HashSet<String>> {
        get_sub_folders(self.folder)
    }

    pub fn get(&mut self, request: &str) -> Result<String, String> {
        let url = request;
        if let Ok(response) = self.get_from_cache(url) {
            println!("retrieving response from cache!");
            Ok(response)
        } else {
            let response = ureq::get(url)
                .call().map_err(|e| e.to_string())?
                .into_string().map_err(|e| e.to_string())?;
            self.put_in_cache(url, String::from(url), response.clone())?;
            Ok(response)
        }
    }

    // hash!
    fn get_hash(&self, request_url: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        request_url.hash(&mut hasher);
        hasher.finish()
    }

    fn get_from_cache(&self, url: &str) -> Result<String, String> {
        let url_hash = self.get_hash(url);
        let dirs = self.get_sub_folders()
            .map_err(|e| format!("Could not obtain top-level subdirectories"))?;
        let hash_name = url_hash.to_string();
        if !dirs.contains(&hash_name) {
            Err(format!("Did not contain hash {}", url_hash))
        } else {
            let chain_index = self.check_subdirs_for_url(url, &hash_name);
            if let Some(i) = chain_index {
                let mut f = OpenOptions::new().read(true)
                    .open(format!("{}/{}/{}/data", self.folder, hash_name, i))
                    .map_err(|e| e.to_string())?;
                let mut s = String::new();
                f.read_to_string(&mut s);
                Ok(s)
            } else {
                // probably remove this later?
                panic!("Cache didn't contain {} even though it contained the hash!", url);
            }
        }
    }

    fn check_subdirs_for_url(&self, url: &str, hash_dir: &String) -> Option<usize> {
        let folder_path = format!("{}/{}", self.folder, hash_dir.as_str());
        let chain = get_sub_folders(folder_path.as_str())
            .ok()?
            .into_iter().map(|dir_name| usize::from_str(&dir_name).unwrap())
            .collect::<Vec<_>>();
        let mut found_url = None;
        'outer:
        for fold_n in chain {
            match OpenOptions::new().read(true).open(
                // todo: hardcoded string?
                format!("{}/{}/{}/key", self.folder, &hash_dir, fold_n)) {
                Ok(mut f) => {
                    let mut content = String::new();
                    f.read_to_string(&mut content);
                    if content.trim() == url {
                        found_url = Some(fold_n);
                        break 'outer;
                    }
                }
                Err(_) => {
                    // it should be able to open
                    // but if it can't, we just skip it, I guess?
                }
            }
        }
        found_url
    }

    fn put_in_cache(&mut self, url: &str, meta: String, data: String) -> Result<(), String> {
        let url_hash = self.get_hash(url);
        let hash_name = format!("{}", url_hash);
        let hash_folders = get_sub_folders(self.folder)
            .map_err(|e| e.to_string())?;
        let hash_dir = format!("{}/{}", self.folder, &hash_name);
        let mut new_entry = false;
        if !hash_folders.contains(&hash_name) {
            std::fs::create_dir(&hash_dir);
            new_entry = true;
        }
        // find the subdirectory name with the largest value, make one larger than it
        let chain = get_sub_folders(hash_dir.as_str())
            .map_err(|e| e.to_string())?
            .into_iter().map(|dir_name| usize::from_str(&dir_name).unwrap())
            .collect::<Vec<_>>();

        // integer symbolizing part in chain (in case 2 hashes are identical)
        let found_url = self.check_subdirs_for_url(url, &hash_name);

        // number of chain in the directory to write to
        let n = found_url
            .or(
                chain.iter().max()
                    .map(|x| x + 1)
            )
            .or(Some(0)).unwrap();
        // 'create' directory in case it doesn't exist
        std::fs::create_dir(format!("{}/{}/{}", self.folder, &hash_name, n));
        // write data to `data` file
        OpenOptions::new().write(true)
            .truncate(true) // clear the file before writing to it
            .create(true)
            .open(
                // todo: hardcoded string?
                format!("{}/{}/{}/data", self.folder, &hash_name, n)
            )
            .map(|mut f| {
                write!(f, "{}", data);
            });

        // write data to `meta` file
        OpenOptions::new().write(true)
            .truncate(true) // clear the file before writing to it
            .create(true)
            .open(
                // todo: hardcoded string?
                format!("{}/{}/{}/key", self.folder, &hash_name, n)
            )
            .map(|mut f| {
                write!(f, "{}", meta);
            });
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::collections::{HashMap, HashSet};
    use crate::server::cache::{Cache, CacheIndex, get_sub_folders};

    #[test]
    fn test_cache_creation () {
        let mut cache = CacheIndex::new("cache/cache-meta").unwrap();
        cache.clear_cache();
        assert_eq!(cache.get_entries(), &HashMap::new());
    }

    #[test]
    fn naive_folder_test() {
        println!("{:?}", get_sub_folders("cache/"));
    }

    #[test]
    fn cache_test() {
        let mut cache = Cache::new(
            "cache/cache-meta/cache-index",
            "cache/data").unwrap();
        println!("{:?}", cache.get("https://en.wikipedia.org/api/rest_v1/page/title/Earth"));
    }
}