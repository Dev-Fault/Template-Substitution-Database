pub use rusqlite;
use rusqlite::{Connection, Transaction};

pub struct TemplateDatabase {
    db: Connection,
}

pub type UpdatedValues<'a> = Vec<&'a str>;

impl TemplateDatabase {
    pub fn from_path(path: &str) -> rusqlite::Result<TemplateDatabase> {
        let db = Connection::open(path)?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS templates (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        )",
            [],
        )?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS substitutes (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            template_id INTEGER NOT NULL REFERENCES templates(id),
            UNIQUE(name, template_id)
        )",
            [],
        )?;

        Ok(TemplateDatabase { db })
    }

    pub fn insert_sub<'a>(
        &mut self,
        template: &'a str,
        substitute: &'a str,
    ) -> rusqlite::Result<bool> {
        let tx = self.db.transaction()?;
        let template_id = TemplateDatabase::find_template_id_with_transaction(&tx, template)?;
        let result = tx.execute(
            "INSERT OR IGNORE INTO substitutes (name, template_id) VALUES (?1, ?2)",
            [substitute.to_string(), template_id.to_string()],
        )?;

        tx.commit()?;

        Ok(result > 0)
    }

    pub fn insert_subs<'a>(
        &mut self,
        template: &'a str,
        substitutes: Option<&[&'a str]>,
    ) -> rusqlite::Result<UpdatedValues<'a>> {
        let mut change_log = UpdatedValues::new();

        let tx = self.db.transaction()?;

        TemplateDatabase::execute_insert_template(&tx, template)?;

        if let Some(subs) = substitutes {
            change_log = TemplateDatabase::execute_insert_subs(&tx, template, subs)?;
        }

        tx.commit()?;

        Ok(change_log)
    }

    fn execute_insert_template(tx: &Transaction, template: &str) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT OR IGNORE INTO templates (name) VALUES (?1)",
            &[template],
        )?;
        Ok(())
    }

    fn execute_insert_subs<'a>(
        tx: &Transaction,
        template: &str,
        substitutes: &[&'a str],
    ) -> rusqlite::Result<UpdatedValues<'a>> {
        let template_id = TemplateDatabase::find_template_id_with_transaction(&tx, template)?;
        let mut inserted_subs = UpdatedValues::new();

        for sub in substitutes {
            let result = tx.execute(
                "INSERT OR IGNORE INTO substitutes (name, template_id) VALUES (?1, ?2)",
                &[*sub, &template_id],
            )?;
            if result > 0 {
                inserted_subs.push(*sub);
            }
        }

        Ok(inserted_subs)
    }

    fn find_template_id(&self, template: &str) -> rusqlite::Result<String> {
        let mut stmt = self
            .db
            .prepare("SELECT id FROM templates WHERE name = ?1")?;
        let template_id: i64 = stmt.query_row(&[template], |row| row.get(0))?;
        Ok(template_id.to_string())
    }

    fn find_template_id_with_transaction(
        tx: &Transaction,
        template: &str,
    ) -> rusqlite::Result<String> {
        let mut stmt = tx.prepare("SELECT id FROM templates WHERE name = ?1")?;
        let template_id: i64 = stmt.query_row(&[template], |row| row.get(0))?;
        Ok(template_id.to_string())
    }

    pub fn remove_template(&mut self, template: &str) -> rusqlite::Result<bool> {
        let tx = self.db.transaction()?;
        let template_id = TemplateDatabase::find_template_id_with_transaction(&tx, template)?;

        tx.execute(
            "DELETE FROM substitutes WHERE template_id = ?1",
            [&template_id],
        )?;

        let result = tx.execute("DELETE FROM templates WHERE id = ?1", [&template_id])?;

        tx.commit()?;

        Ok(result > 0)
    }

    pub fn remove_sub<'a>(
        &mut self,
        template: &'a str,
        substitute: &'a str,
    ) -> rusqlite::Result<bool> {
        let tx = self.db.transaction()?;
        let template_id = TemplateDatabase::find_template_id_with_transaction(&tx, template)?;

        let result = tx.execute(
            "DELETE FROM substitutes WHERE template_id = ?1 AND name = ?2",
            &[&template_id, substitute],
        )?;

        tx.commit()?;

        Ok(result > 0)
    }

    pub fn remove_subs<'a>(
        &mut self,
        template: &'a str,
        substitutes: &[&'a str],
    ) -> rusqlite::Result<UpdatedValues<'a>> {
        let tx = self.db.transaction()?;
        let template_id = TemplateDatabase::find_template_id_with_transaction(&tx, template)?;

        let mut removed_subs = UpdatedValues::new();

        for sub in substitutes {
            let result = tx.execute(
                "DELETE FROM substitutes WHERE template_id = ?1 AND name = ?2",
                &[&template_id, *sub],
            )?;
            if result > 0 {
                removed_subs.push(*sub);
            }
        }

        tx.commit()?;

        Ok(removed_subs)
    }

    pub fn rename_template(
        &mut self,
        old_template: &str,
        new_template: &str,
    ) -> rusqlite::Result<bool> {
        let tx = self.db.transaction()?;

        let result = tx.execute(
            "UPDATE templates SET name = ?1 WHERE name = ?2",
            &[new_template, old_template],
        )?;

        tx.commit()?;

        Ok(result > 0)
    }

    pub fn rename_substitute(
        &mut self,
        template: &str,
        old_sub: &str,
        new_sub: &str,
    ) -> rusqlite::Result<bool> {
        let tx = self.db.transaction()?;

        let template_id = TemplateDatabase::find_template_id_with_transaction(&tx, template)?;

        let result = tx.execute(
            "UPDATE substitutes SET name = ?1 WHERE name = ?2 AND template_id = ?3",
            &[new_sub, old_sub, &template_id],
        )?;

        tx.commit()?;

        Ok(result > 0)
    }

    pub fn clear(&self) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM substitutes", [])?;
        self.db.execute("DELETE FROM templates", [])?;
        Ok(())
    }

    pub fn get_subs(&self, template: &str) -> rusqlite::Result<Vec<String>> {
        let template_id = self.find_template_id(template)?;
        let mut stmt = self.db.prepare(
            "SELECT substitutes.name
             FROM substitutes
             WHERE template_id = ?1
             ORDER BY LOWER(substitutes.name) ASC;",
        )?;

        let substitutes = stmt.query_map([template_id], |row| row.get(0))?;

        Ok(substitutes
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap())
            .collect())
    }

    pub fn get_random_subs(&self, template: &str) -> rusqlite::Result<String> {
        let template_id = self.find_template_id(template)?;
        let mut stmt = self.db.prepare(
            "SELECT substitutes.name
             FROM substitutes
             WHERE template_id = ?1
             ORDER BY RANDOM() LIMIT 1;",
        )?;

        let mut rows = stmt.query([template_id])?;

        match rows.next()? {
            Some(row) => {
                let sub: String = row.get(0)?;
                return Ok(sub);
            }
            _ => Ok("".to_string()),
        }
    }

    pub fn get_templates(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.db.prepare(
            "SELECT templates.name
             FROM templates
             ORDER BY LOWER(templates.name) ASC;",
        )?;

        let templates = stmt.query_map([], |row| row.get(0))?;

        Ok(templates
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    const NOUNS: &[&str] = &[
        "cat",
        "dog",
        "tree",
        "cup",
        "pencil",
        "desk",
        "man",
        "woman",
        "ape",
        "bed",
        "Africa",
        "United States",
    ];

    const VERBS: &[&str] = &[
        "run", "jump", "hide", "fly", "cry", "kill", "throw", "catch", "eat", "arrest", "find",
        "slide",
    ];

    const ADJECTIVES: &[&str] = &[
        "funny",
        "cool",
        "mean",
        "jovial",
        "jerkish",
        "excellent",
        "great",
        "bad",
        "ripe",
        "jumpy",
        "fragmented",
        "untolerable",
    ];

    #[should_panic]
    #[test]
    fn get_inside_empty_database() {
        let db = TemplateDatabase::from_path("test1.db").unwrap();

        db.get_subs("noun").unwrap();
    }

    #[test]
    fn insert_new_templates_with_subtitutions() {
        let mut db = TemplateDatabase::from_path("test2.db").unwrap();

        db.insert_subs("noun", Some(NOUNS)).unwrap();
        db.insert_subs("verb", Some(VERBS)).unwrap();
        db.insert_subs("adj", Some(ADJECTIVES)).unwrap();

        let templates = db.get_templates().unwrap();
        let noun_subs = db.get_subs("noun").unwrap();
        let verb_subs = db.get_subs("verb").unwrap();
        let adj_subs = db.get_subs("adj").unwrap();

        assert!(templates.contains(&"noun".to_string()));
        assert!(templates.contains(&"adj".to_string()));
        assert!(templates.contains(&"verb".to_string()));
        for noun in NOUNS {
            assert!(noun_subs.contains(&noun.to_string()));
        }
        for verb in VERBS {
            assert!(verb_subs.contains(&verb.to_string()));
        }
        for adj in ADJECTIVES {
            assert!(adj_subs.contains(&adj.to_string()));
        }
    }

    #[test]
    fn insert_only_template() {
        let mut db = TemplateDatabase::from_path("test4.db").unwrap();

        db.insert_subs("template-with-no-subs", Some(&[])).unwrap();

        let empty: Vec<String> = Vec::new();
        assert_eq!(db.get_subs("template-with-no-subs").unwrap(), empty);
    }

    #[test]
    fn remove_substitutes() {
        let mut db = TemplateDatabase::from_path("test5.db").unwrap();

        db.insert_subs("noun", Some(NOUNS)).unwrap();

        assert_eq!(db.get_subs("noun").unwrap().len(), NOUNS.len());

        let empty: Vec<String> = Vec::new();

        db.remove_subs("noun", NOUNS).unwrap();

        assert_eq!(db.get_subs("noun").unwrap(), empty);

        db.insert_subs("verb", Some(VERBS)).unwrap();

        assert_eq!(db.get_subs("verb").unwrap().len(), VERBS.len());

        db.remove_subs("verb", &["JAFLJE;LSFKALESF"]).unwrap();

        db.remove_subs("verb", &["jump"]).unwrap();

        assert!(!db.get_subs("verb").unwrap().contains(&"jump".to_string()));
    }

    #[test]
    fn remove_template() {
        let mut db = TemplateDatabase::from_path("test6.db").unwrap();

        db.insert_subs("noun", Some(NOUNS)).unwrap();

        assert_eq!(db.get_subs("noun").unwrap().len(), NOUNS.len());

        db.remove_template("noun").unwrap();

        assert!(!db.get_templates().unwrap().contains(&"noun".to_string()));
    }

    #[test]
    fn remove_non_existant_template() {
        let mut db = TemplateDatabase::from_path("test6.db").unwrap();

        match db.remove_template("noun") {
            Ok(_) => {}
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                dbg!("Ignoring query returned no rows error...");
            }
            Err(err) => {
                eprintln!("Error: {}", err);
            }
        }

        assert!(!db.get_templates().unwrap().contains(&"noun".to_string()));
    }

    #[test]
    fn rename_template() {
        let mut db = TemplateDatabase::from_path("test7.db").unwrap();

        db.clear().unwrap();

        db.insert_subs("noun", Some(NOUNS)).unwrap();

        db.rename_template("noun", "new-nouns").unwrap();

        assert_eq!(db.get_templates().unwrap(), vec!["new-nouns"]);
    }

    #[test]
    fn insert_substitutes_with_same_name() {
        let mut db = TemplateDatabase::from_path("test8.db").unwrap();

        db.clear().unwrap();

        db.insert_subs("noun", Some(&["example", "example2"]))
            .unwrap();

        db.insert_subs("noun2", Some(&["example", "example2"]))
            .unwrap();
    }

    #[test]
    fn insert_substitutes_with_same_name_with_same_template() {
        let mut db = TemplateDatabase::from_path("test9.db").unwrap();

        db.clear().unwrap();

        db.insert_subs("noun", Some(&["example", "example2"]))
            .unwrap();

        db.insert_subs("noun", Some(&["example", "example2"]))
            .unwrap();

        assert_eq!(db.get_subs("noun").unwrap(), &["example", "example2"]);
    }
}
