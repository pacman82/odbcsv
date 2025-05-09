use anyhow::Error;
use assert_cmd::{Command, assert::Assert};
use odbc_api::{Connection, ConnectionOptions, Environment};
use std::{
    fs::{self, File},
    io::Read,
    sync::OnceLock,
};
use tempfile::NamedTempFile;

const MSSQL: &str = "Driver={ODBC Driver 18 for SQL Server};Server=localhost;UID=SA;PWD=My@Test@Password1;\
    TrustServerCertificate=yes;";

#[cfg(target_os = "windows")]
const MARIADB: &str = "Driver={MariaDB ODBC 3.1 Driver};\
    Server=localhost;DB=test_db;\
    UID=root;PWD=my-secret-pw;\
    Port=3306";

// Use 127.0.0.1 instead of localhost so the system uses the TCP/IP connector instead of the socket
// connector. Prevents error message: 'Can't connect to local MySQL server through socket'.
#[cfg(not(target_os = "windows"))]
const MARIADB: &str = "Driver={MariaDB Unicode};\
    Server=127.0.0.1;DB=test_db;\
    UID=root;PWD=my-secret-pw;\
    Port=3306";

/// We share one environment for all tests.
static ENV: OnceLock<Environment> = OnceLock::new();

/// A reference to the ODBC environment with static lifetime.
pub fn env() -> &'static Environment {
    if let Some(env) = ENV.get() {
        env
    } else {
        let env = Environment::new().expect("Must be able to create ODBC Environment");
        let odbc_env = ENV.get_or_init(|| env);
        odbc_env
    }
}

/// Test helper using two commands to roundtrip csv to and from a data source.
///
/// # Parameters
///
/// * `csv`: csv used in the roundtrip. Table schema is currently hardcoded.
/// * `table_name`: Each test must use its unique table name, to avoid race conditions with other
///   tests.
/// * `batch_size`: Batch size for insert
fn roundtrip(csv: &'static str, table_name: &str, batch_size: u32) -> Assert {
    // Setup table for test. We use the table name only in this test.
    let conn = env()
        .connect_with_connection_string(MSSQL, ConnectionOptions::default())
        .unwrap();
    conn.execute(&format!("DROP TABLE IF EXISTS {table_name}"), (), None)
        .unwrap();
    conn.execute(
        &format!("CREATE TABLE {table_name} (country VARCHAR(255), population BIGINT);"),
        (),
        None,
    )
    .unwrap();

    // Insert csv
    Command::cargo_bin("odbcsv")
        .unwrap()
        .args([
            "-vvvv",
            "insert",
            "--connection-string",
            MSSQL,
            "--batch-size",
            &batch_size.to_string(),
            table_name,
        ])
        .write_stdin(csv)
        .assert()
        .success();

    // Query csv
    Command::cargo_bin("odbcsv")
        .unwrap()
        .args([
            "-vvvv",
            "query",
            "--connection-string",
            MSSQL,
            &format!("SELECT country, population FROM {table_name} ORDER BY population;"),
        ])
        .assert()
        .stdout(csv)
}

/// Query MSSQL database, yet do not specify username and password in the connection string, but
/// pass them as separate command line options.
#[test]
fn append_user_and_password_to_connection_string() -> Result<(), Error> {
    // Connection string without user name and password.
    let connection_string =
        "Driver={ODBC Driver 18 for SQL Server};Server=localhost;TrustServerCertificate=yes;";

    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "query",
            "--connection-string",
            connection_string,
            "--user",
            "SA",
            "--password",
            "My@Test@Password1",
            "SELECT 42",
        ])
        .assert()
        .success();
    Ok(())
}

#[test]
fn query_mssql() -> Result<(), Error> {
    let table_name = "OdbcsvQueryMssql";
    let conn = env().connect_with_connection_string(MSSQL, ConnectionOptions::default())?;
    setup_empty_table(&conn, table_name, &["VARCHAR(255) NOT NULL", "INT"])?;
    let insert = format!(
        "INSERT INTO {table_name}
        (a, b)
        Values
        ('Jurassic Park', 1993),
        ('2001: A Space Odyssey', 1968),
        ('Interstellar', NULL);"
    );
    conn.execute(&insert, (), None)?;

    let csv = "a,b\n\
        Jurassic Park,1993\n\
        2001: A Space Odyssey,1968\n\
        Interstellar,\n\
    ";

    let query = format!("SELECT a, b from {table_name}");
    Command::cargo_bin("odbcsv")
        .unwrap()
        .args(["-vvvv", "query", "--connection-string", MSSQL, &query])
        .assert()
        .success()
        .stdout(csv);
    Ok(())
}

#[test]
fn tables() -> Result<(), Error> {
    let csv = "TABLE_CAT,TABLE_SCHEM,TABLE_NAME,TABLE_TYPE,REMARKS\n\
        master,dbo,OdbcsvTestTables,TABLE,\n\
    ";

    let table_name = "OdbcsvTestTables";
    // Setup table for test. We use the table name only in this test.
    let conn = env().connect_with_connection_string(MSSQL, ConnectionOptions::default())?;
    setup_empty_table(&conn, table_name, &["INTEGER"])?;

    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "list-tables",
            "--connection-string",
            MSSQL,
            "--name",
            table_name,
        ])
        .assert()
        .success()
        .stdout(csv);
    Ok(())
}

#[test]
fn columns() -> Result<(), Error> {
    let csv = "TABLE_CAT,TABLE_SCHEM,TABLE_NAME,COLUMN_NAME,DATA_TYPE,TYPE_NAME,COLUMN_SIZE,\
    BUFFER_LENGTH,DECIMAL_DIGITS,NUM_PREC_RADIX,NULLABLE,REMARKS,COLUMN_DEF,SQL_DATA_TYPE,\
    SQL_DATETIME_SUB,CHAR_OCTET_LENGTH,ORDINAL_POSITION,IS_NULLABLE,SS_IS_SPARSE,SS_IS_COLUMN_SET,\
    SS_IS_COMPUTED,SS_IS_IDENTITY,SS_UDT_CATALOG_NAME,SS_UDT_SCHEMA_NAME,SS_UDT_ASSEMBLY_TYPE_NAME,\
    SS_XML_SCHEMACOLLECTION_CATALOG_NAME,SS_XML_SCHEMACOLLECTION_SCHEMA_NAME,\
    SS_XML_SCHEMACOLLECTION_NAME,SS_DATA_TYPE\n\
    master,dbo,OdbcsvTestColumns,a,12,varchar,255,255,,,1,,,12,,255,1,YES,0,0,0,0,,,,,,,39\n\
    ";

    let table_name = "OdbcsvTestColumns";
    // Setup table for test. We use the table name only in this test.
    // Setup empty table handle would implicitly create an ID column
    let conn = env().connect_with_connection_string(MSSQL, ConnectionOptions::default())?;
    conn.execute(&format!("DROP TABLE IF EXISTS {table_name}"), (), None)?;
    conn.execute(
        &format!("CREATE TABLE {table_name} (a VARCHAR(255));"),
        (),
        None,
    )?;

    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "list-columns",
            "--connection-string",
            MSSQL,
            "--catalog",
            "master",
            "--table",
            table_name,
        ])
        .assert()
        .success()
        .stdout(csv);
    Ok(())
}

#[test]
fn ignore_truncation() -> Result<(), Error> {
    let csv = "some_string\n\
        1234\n\
    ";

    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "query",
            "--max-str-len",
            "4",
            "--ignore-truncation",
            "--connection-string",
            MSSQL,
            "SELECT '12345' as some_string",
        ])
        .assert()
        .success()
        .stdout(csv);
    Ok(())
}

#[test]
fn do_not_ignore_truncation() -> Result<(), Error> {
    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "query",
            "--max-str-len",
            "4",
            "--connection-string",
            MSSQL,
            "SELECT '12345' as some_string",
        ])
        .assert()
        .failure();
    Ok(())
}

#[test]
fn placeholders() -> Result<(), Error> {
    let table_name = "OdbcsvPlaceholders";
    let conn = env().connect_with_connection_string(MSSQL, ConnectionOptions::default())?;
    setup_empty_table(&conn, table_name, &["VARCHAR(255) NOT NULL", "INT"])?;
    let insert = format!(
        "INSERT INTO {table_name}
        (a, b)
        Values
        ('one', 10),
        ('two', 20),
        ('thre', NULL);"
    );
    conn.execute(&insert, (), None)?;

    let csv = "a\n\
        two\n\
    ";

    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "query",
            "--connection-string",
            MSSQL,
            &format!("SELECT a from {table_name} where b > ? and b < ?;"),
            "12",
            "23",
        ])
        .assert()
        .success()
        .stdout(csv);
    Ok(())
}

#[test]
fn insert() {
    let csv = "country,population\n\
        Norway,5380000\n\
        Germany,83500000\n\
        USA,329000000\n\
    ";

    roundtrip(csv, "odbcsv_insert", 5).success();
}

#[test]
fn insert_empty_document() {
    let csv = "country,population\n";

    roundtrip(csv, "odbcsv_empty_document", 5).success();
}

#[test]
fn insert_batch_size_one() {
    let csv = "country,population\n\
        Norway,5380000\n\
        Germany,83500000\n\
        USA,329000000\n\
    ";

    roundtrip(csv, "odbcsv_insert_batch_size_one", 1).success();
}

#[test]
fn insert_with_nulls() {
    let csv = "country,population\n\
        Norway,\n\
        ,83500000\n\
        USA,329000000\n\
    ";

    roundtrip(csv, "odbcsv_insert_with_nulls", 5).success();
}

/// An "optional" for list-drivers command. It checks for the existence of a "list-drivers.txt". If
/// so it compares the output of the `list-drivers` command with the file content. This setup is
/// intended to provide a test for dev container or CI setups there the installed drivers are
/// controlled by this repository, but gracefully skip, if we run natively on a developer machine
/// with a different set of drivers installed.
#[test]
fn list_drivers() -> Result<(), Error> {
    if let Ok(mut expectation_file) = File::open("tests/list-drivers.txt") {
        let mut expectations = String::new();
        expectation_file.read_to_string(&mut expectations)?;

        let mut command = Command::cargo_bin("odbcsv")?;
        let odbcsv = command.args(["-vvvv", "list-drivers"]);
        odbcsv.assert().success();
        let output = String::from_utf8(odbcsv.output()?.stdout)?;

        let installed_drivers: Vec<&str> = output
            .lines()
            .filter(|&maybe_driver| {
                // Only look at the driver names, no need to check for descriptions (parameters are indented)
                !maybe_driver.is_empty() && !maybe_driver.starts_with(&[' ', '\t'][..])
            })
            .collect();

        let not_configured_drivers: Vec<&str> = expectations
            .trim_end()
            .lines()
            .filter(|driver| !installed_drivers.contains(driver))
            .collect();

        if !not_configured_drivers.is_empty() {
            panic!(
                "'{}' drivers are not configured in the system",
                not_configured_drivers.join(", ")
            );
        }
    }

    Ok(())
}

/// Creates the table and assures it is empty. Columns are named a,b,c, etc.
pub fn setup_empty_table(
    conn: &Connection<'_>,
    table_name: &str,
    column_types: &[&str],
) -> Result<(), odbc_api::Error> {
    let drop_table = &format!("DROP TABLE IF EXISTS {table_name}");

    let column_names = &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"];
    let cols = column_types
        .iter()
        .zip(column_names)
        .map(|(ty, name)| format!("{name} {ty}"))
        .collect::<Vec<_>>()
        .join(", ");

    let create_table = format!("CREATE TABLE {table_name} (id int IDENTITY(1,1),{cols});");
    conn.execute(drop_table, (), None)?;
    conn.execute(&create_table, (), None)?;
    Ok(())
}

#[test]
fn fetch_from_mssql() {
    let table_name = "OdbcsvFetchMssql";
    let conn = env()
        .connect_with_connection_string(MSSQL, ConnectionOptions::default())
        .unwrap();
    setup_empty_table(&conn, table_name, &["VARCHAR(255) NOT NULL", "INT"]).unwrap();
    let insert = format!(
        "INSERT INTO {table_name}
        (a, b)
        Values
        ('Jurassic Park', 1993),
        ('2001: A Space Odyssey', 1968),
        ('Interstellar', NULL);"
    );
    conn.execute(&insert, (), None).unwrap();

    let csv = "a,b\n\
        Jurassic Park,1993\n\
        2001: A Space Odyssey,1968\n\
        Interstellar,\n\
    ";

    let query = format!("SELECT a, b from {table_name}");
    Command::cargo_bin("odbcsv")
        .unwrap()
        .args([
            "-vvvv",
            "fetch",
            "--connection-string",
            MSSQL,
            "--query",
            &query,
        ])
        .assert()
        .success()
        .stdout(csv);
}

#[test]
fn fetch_with_query_read_from_file() -> Result<(), Error> {
    // Fill Table with dummy data
    let table_name = "OdbcsvFetchWithQueryReadFromFile";
    let conn = env().connect_with_connection_string(MSSQL, ConnectionOptions::default())?;
    setup_empty_table(&conn, table_name, &["VARCHAR(255) NOT NULL", "INT"])?;
    let insert = format!(
        "INSERT INTO {table_name}
        (a, b)
        Values
        ('Jurassic Park', 1993),
        ('2001: A Space Odyssey', 1968),
        ('Interstellar', NULL);"
    );
    conn.execute(&insert, (), None)?;

    // Write query into temporary file
    let named = NamedTempFile::new()?;
    let path = named.into_temp_path();
    let query = format!("SELECT a, b from {table_name}");
    fs::write(&path, query)?;

    // Use query safed in file to fetch dummy data and assert the result
    let csv = "a,b\n\
        Jurassic Park,1993\n\
        2001: A Space Odyssey,1968\n\
        Interstellar,\n\
    ";

    Command::cargo_bin("odbcsv")?
        .args([
            "-vvvv",
            "fetch",
            "--connection-string",
            MSSQL,
            "--sql-file",
            path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(csv);
    Ok(())
}

#[test]
fn list_columns_with_maria_db() {
    // Maria DB driver reports very large column sizes, likely to cause an out of memory if just
    // allocated.
    Command::cargo_bin("odbcsv")
        .unwrap()
        .args(["-vvvv", "list-columns", "--connection-string", MARIADB])
        .assert()
        .success();
}
