use rusqlite::{Connection, Transaction};

pub struct TemplateDatabase {
    db: Connection,
}

impl TemplateDatabase {
    pub fn from_path(path: &str) -> rusqlite::Result<TemplateDatabase> {
        let db = Connection::open(path)?;

        db.execute(
            "create table if not exists templates (
            id integer primary key,
            name text not null unique
        )",
            [],
        )?;

        db.execute(
            "create table if not exists substitutes (
            id integer primary key,
            name text not null,
            template_id integer not null references templates(id),
            UNIQUE(name, template_id)
        )",
            [],
        )?;

        Ok(TemplateDatabase { db })
    }

    pub fn insert_substitutions(
        &mut self,
        template: &str,
        substitutes: Option<&[&str]>,
    ) -> rusqlite::Result<()> {
        if template.len() == 0 {
            return Ok(());
        }

        let tx = self.db.transaction()?;

        TemplateDatabase::execute_insert_template(&tx, template)?;

        if let Some(subs) = substitutes {
            TemplateDatabase::execute_insert_substitutions(&tx, template, subs)?;
        }

        tx.commit()
    }

    fn execute_insert_template(tx: &Transaction, template: &str) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT OR IGNORE INTO templates (name) VALUES (?1)",
            &[template],
        )?;
        Ok(())
    }

    fn execute_insert_substitutions(
        tx: &Transaction,
        template: &str,
        substitutes: &[&str],
    ) -> rusqlite::Result<()> {
        let template_id = TemplateDatabase::find_template_id(&tx, template)?;

        for sub in substitutes {
            tx.execute(
                "INSERT OR IGNORE INTO substitutes (name, template_id) VALUES (?1, ?2)",
                &[*sub, &template_id],
            )?;
        }

        Ok(())
    }

    fn find_template_id(tx: &Transaction, template: &str) -> rusqlite::Result<String> {
        let mut stmt = tx.prepare("SELECT id FROM templates WHERE name = ?1")?;
        let template_id: i64 = stmt.query_row(&[template], |row| row.get(0))?;
        Ok(template_id.to_string())
    }

    pub fn remove_template(&mut self, template: &str) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        let template_id = TemplateDatabase::find_template_id(&tx, template)?;

        tx.execute(
            "DELETE FROM substitutes WHERE template_id = ?1",
            [&template_id],
        )?;

        tx.execute("DELETE FROM templates WHERE id = ?1", [&template_id])?;

        tx.commit()
    }

    pub fn remove_substitutes(
        &mut self,
        template: &str,
        substitutes: &[&str],
    ) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        let template_id = TemplateDatabase::find_template_id(&tx, template)?;

        for sub in substitutes {
            tx.execute(
                "DELETE FROM substitutes WHERE template_id = ?1 AND name = ?2",
                &[&template_id, &sub.to_string()],
            )?;
        }

        tx.commit()
    }

    pub fn rename_template(
        &mut self,
        old_template: &str,
        new_template: &str,
    ) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;

        tx.execute(
            "UPDATE templates SET name = ?1 WHERE name = ?2",
            &[new_template, old_template],
        )?;

        tx.commit()
    }

    pub fn rename_substitute(
        &mut self,
        template: &str,
        old_sub: &str,
        new_sub: &str,
    ) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;

        let template_id = TemplateDatabase::find_template_id(&tx, template)?;

        tx.execute(
            "UPDATE substitutes SET name = ?1 WHERE name = ?2 AND template_id = ?3",
            &[new_sub, old_sub, &template_id],
        )?;

        tx.commit()
    }

    pub fn clear(&self) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM substitutes", [])?;
        self.db.execute("DELETE FROM templates", [])?;
        Ok(())
    }

    pub fn get_substitutes(&self, template: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.db.prepare(
            "SELECT substitutes.name
             FROM substitutes
             INNER JOIN templates
             ON templates.id = substitutes.template_id
             WHERE templates.name = ?1
             ORDER BY LOWER(substitutes.name) ASC;",
        )?;

        let substitutes = stmt.query_map(&[template], |row| row.get(0))?;

        let mut sub_vec = Vec::new();

        for sub in substitutes {
            sub_vec.push(sub?);
        }

        Ok(sub_vec)
    }

    pub fn get_templates(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.db.prepare(
            "SELECT templates.name
             FROM templates
             ORDER BY LOWER(templates.name) ASC;",
        )?;

        let templates = stmt.query_map([], |row| row.get(0))?;

        let mut template_vec = Vec::new();

        for template in templates {
            template_vec.push(template?);
        }

        Ok(template_vec)
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

    #[test]
    fn get_inside_empty_database() {
        let db = TemplateDatabase::from_path("test1.db").unwrap();

        let empty: Vec<String> = Vec::new();
        assert_eq!(db.get_substitutes("noun").unwrap(), empty);
        assert_eq!(db.get_templates().unwrap(), empty);
    }

    #[test]
    fn insert_new_templates_with_subtitutions() {
        let mut db = TemplateDatabase::from_path("test2.db").unwrap();

        db.insert_substitutions("noun", Some(NOUNS)).unwrap();
        db.insert_substitutions("verb", Some(VERBS)).unwrap();
        db.insert_substitutions("adj", Some(ADJECTIVES)).unwrap();

        assert_eq!(db.get_templates().unwrap(), vec!["noun", "verb", "adj"]);
        assert_eq!(db.get_substitutes("noun").unwrap(), NOUNS);
        assert_eq!(db.get_substitutes("verb").unwrap(), VERBS);
        assert_eq!(db.get_substitutes("adj").unwrap(), ADJECTIVES);
    }

    #[test]
    fn attempt_to_insert_empty_template() {
        let mut db = TemplateDatabase::from_path("test3.db").unwrap();

        db.insert_substitutions("", Some(&["slap"])).unwrap();

        let empty: Vec<String> = Vec::new();

        assert_eq!(empty, db.get_templates().unwrap());
    }

    #[test]
    fn insert_only_template() {
        let mut db = TemplateDatabase::from_path("test4.db").unwrap();

        db.insert_substitutions("template-with-no-subs", Some(&[]))
            .unwrap();

        let empty: Vec<String> = Vec::new();
        assert_eq!(db.get_substitutes("template-with-no-subs").unwrap(), empty);
    }

    #[test]
    fn remove_substitutes() {
        let mut db = TemplateDatabase::from_path("test5.db").unwrap();

        db.insert_substitutions("noun", Some(NOUNS)).unwrap();

        assert_eq!(db.get_substitutes("noun").unwrap().len(), NOUNS.len());

        let empty: Vec<String> = Vec::new();

        db.remove_substitutes("noun", NOUNS).unwrap();

        assert_eq!(db.get_substitutes("noun").unwrap(), empty);

        db.insert_substitutions("verb", Some(VERBS)).unwrap();

        assert_eq!(db.get_substitutes("verb").unwrap().len(), VERBS.len());

        db.remove_substitutes("verb", &["JAFLJE;LSFKALESF"])
            .unwrap();

        db.remove_substitutes("verb", &["jump"]).unwrap();

        assert!(!db
            .get_substitutes("verb")
            .unwrap()
            .contains(&"jump".to_string()));
    }

    #[test]
    fn remove_template() {
        let mut db = TemplateDatabase::from_path("test6.db").unwrap();

        db.insert_substitutions("noun", Some(NOUNS)).unwrap();

        assert_eq!(db.get_substitutes("noun").unwrap().len(), NOUNS.len());

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

        db.insert_substitutions("noun", Some(NOUNS)).unwrap();

        db.rename_template("noun", "new-nouns").unwrap();

        assert_eq!(db.get_templates().unwrap(), vec!["new-nouns"]);
    }

    #[test]
    fn insert_substitutes_with_same_name() {
        let mut db = TemplateDatabase::from_path("test8.db").unwrap();

        db.clear().unwrap();

        db.insert_substitutions("noun", Some(&["example", "example2"]))
            .unwrap();

        db.insert_substitutions("noun2", Some(&["example", "example2"]))
            .unwrap();
    }

    #[test]
    fn insert_substitutes_with_same_name_with_same_template() {
        let mut db = TemplateDatabase::from_path("test9.db").unwrap();

        db.clear().unwrap();

        db.insert_substitutions("noun", Some(&["example", "example2"]))
            .unwrap();

        db.insert_substitutions("noun", Some(&["example", "example2"]))
            .unwrap();

        assert_eq!(
            db.get_substitutes("noun").unwrap(),
            &["example", "example2"]
        );
    }
}
