



fn reader() -> Result<Vec<TaskRecord>, Box<dyn Error>> {
    // fn reader() -> Result<Vec<TaskRecord>, Error> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b'\t')
            .trim(csv::Trim::All)
            .from_reader(io::stdin());
            // to-od: Overwrite it to handle errors: https://doc.rust-lang.org/rust-by-example/error/iter_result.html
        let result = rdr.deserialize().map(|r| r.unwrap()).collect::<Vec<TaskRecord>>();
        Ok(result)
    }
    