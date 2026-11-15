use super::*;

// A packaged bootstrap is required by component Electron distributions that do
// not search resources/app. Keep the vendor archive and account data untouched.
pub fn ensure_bootstrap(resources: &Path) -> Result<()> {
    let mut files=serde_json::Map::new();
    let mut data=Vec::new();
    for name in ["package.json","index.cjs","profile.json"] {
        let bytes=fs::read(resources.join("app").join(name)).explain()?;
        files.insert(name.into(),serde_json::json!({"size":bytes.len(),"offset":data.len().to_string()}));
        data.extend(bytes);
    }
    let json=serde_json::to_vec(&serde_json::json!({"files":files})).explain()?;
    let payload=(4+json.len()+3)&!3;
    let header=4+payload;
    let mut archive=Vec::new();
    for value in [4,header as u32,payload as u32,json.len() as u32] {archive.extend(value.to_le_bytes());}
    archive.extend(json);archive.resize(8+header,0);archive.extend(data);
    let target=resources.join("app.asar");
    if fs::read(&target).ok().as_deref()==Some(archive.as_slice()){return Ok(());}
    let temp=resources.join("orbit-bootstrap.tmp");
    fs::write(&temp,archive).explain()?;
    fs::rename(temp,target).map_err(|e|format!("Close this clone before updating its startup package: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_archive_contains_exact_entrypoint_and_profile() {
        let root=std::env::temp_dir().join(format!("orbit-archive-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("app")).unwrap();
        let entries=[("package.json",r#"{"main":"index.cjs"}"#),("index.cjs","// non-aligned entrypoint\n"),("profile.json",r#"{"name":"نسخهٔ ۳"}"#)];
        for (name,value) in entries {fs::write(root.join("app").join(name),value).unwrap();}
        ensure_bootstrap(&root).unwrap();
        let bytes=fs::read(root.join("app.asar")).unwrap();
        let read=|offset|u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()) as usize;
        assert_eq!(read(0),4);let base=8+read(4);
        let header:serde_json::Value=serde_json::from_slice(&bytes[16..16+read(12)]).unwrap();
        for (name,value) in entries {let e=&header["files"][name];let start=base+e["offset"].as_str().unwrap().parse::<usize>().unwrap();let size=e["size"].as_u64().unwrap() as usize;assert_eq!(&bytes[start..start+size],value.as_bytes());}
        ensure_bootstrap(&root).unwrap();assert_eq!(bytes,fs::read(root.join("app.asar")).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
