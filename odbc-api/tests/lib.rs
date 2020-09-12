use buffers::ColumnBuffer;
use env_logger;
use lazy_static::lazy_static;
use odbc_api::{buffers, sys::SqlDataType, ColumnDescription, Environment, Nullable, U16String};
use odbc_sys::Integer;
use std::{convert::TryInto, sync::Mutex};

const MSSQL: &str =
    "Driver={ODBC Driver 17 for SQL Server};Server=localhost;UID=SA;PWD=<YourStrong@Passw0rd>;";

// Rust by default executes tests in parallel. Yet only one environment is allowed at a time.
// Therefore synchronize test execution.
lazy_static! {
    static ref SERIALIZE: Mutex<()> = Mutex::new(());
}

fn init() -> &'static Mutex<()> {
    // Set environment to something like:
    // RUST_LOG=odbc-api=info cargo test
    let _ = env_logger::builder().is_test(true).try_init();
    &SERIALIZE
}

#[test]
fn bogus_connection_string() {
    let _ = init().lock();
    let env = unsafe { Environment::new().unwrap() };
    let conn = env.connect_with_connection_string("foobar");
    assert!(matches!(conn, Err(_)));
}

#[test]
fn connect_to_movies_db() {
    let _ = init().lock();
    let env = unsafe { Environment::new().unwrap() };
    let _conn = env.connect_with_connection_string(MSSQL).unwrap();
}

#[test]
fn mssql_describe_columns() {
    let _ = init().lock();
    let env = unsafe { Environment::new().unwrap() };

    let mut conn = env.connect_with_connection_string(MSSQL).unwrap();
    let sql = "SELECT title, year FROM Movies ORDER BY year;";
    let cursor = conn.exec_direct(sql).unwrap().unwrap();

    assert_eq!(cursor.num_result_cols().unwrap(), 2);
    let mut cd = ColumnDescription::default();
    cursor.describe_col(1, &mut cd).unwrap();

    cursor.describe_col(1, &mut cd).unwrap();
    let name = U16String::from_str("title");

    // Expectation title column
    let title_desc = ColumnDescription {
        name: name.into_vec(),
        data_type: odbc_sys::SqlDataType::VARCHAR,
        column_size: 255,
        decimal_digits: 0,
        nullable: Nullable::NoNulls,
    };

    assert_eq!(title_desc, cd);

    cursor.describe_col(2, &mut cd).unwrap();
    let name = U16String::from_str("year");

    // Expectation title column
    let year_desc = ColumnDescription {
        name: name.into_vec(),
        data_type: odbc_sys::SqlDataType::INTEGER,
        column_size: 10,
        decimal_digits: 0,
        nullable: Nullable::Nullable,
    };

    assert_eq!(year_desc, cd);
}

#[test]
fn mssql_text_buffer() {
    let _ = init().lock();
    let env = unsafe { Environment::new().unwrap() };

    let mut conn = env.connect_with_connection_string(MSSQL).unwrap();
    let sql = "SELECT title, year FROM Movies ORDER BY year;";
    let cursor = conn.exec_direct(sql).unwrap().unwrap();

    let batch_size = 2;
    let mut buffer = buffers::TextRowSet::new(batch_size, &cursor).unwrap();
    let mut row_set_cursor = cursor.bind_row_set_buffer(&mut buffer).unwrap();
    let mut row_set = row_set_cursor.fetch().unwrap().unwrap();
    assert_eq!(row_set.at_as_str(0, 0).unwrap().unwrap(), "Interstellar");
    assert!(row_set.at_as_str(1, 0).unwrap().is_none());
    assert_eq!(
        row_set.at_as_str(0, 1).unwrap().unwrap(),
        "2001: A Space Odyssey"
    );
    assert_eq!(row_set.at_as_str(1, 1).unwrap().unwrap(), "1968");
    row_set = row_set_cursor.fetch().unwrap().unwrap();
    assert_eq!(row_set.at_as_str(0, 0).unwrap().unwrap(), "Jurassic Park");
    assert_eq!(row_set.at_as_str(1, 0).unwrap().unwrap(), "1993");
}

#[test]
fn mssql_column_attributes() {
    let _ = init().lock();
    let env = unsafe { Environment::new().unwrap() };

    let mut conn = env.connect_with_connection_string(MSSQL).unwrap();
    let sql = "SELECT title, year FROM Movies;";
    let cursor = conn.exec_direct(sql).unwrap().unwrap();

    let mut buf = Vec::new();

    cursor.col_name(1, &mut buf).unwrap();
    let buf = U16String::from_vec(buf);
    assert_eq!("title", buf.to_string().unwrap());

    let mut buf = buf.into_vec();
    cursor.col_name(2, &mut buf).unwrap();
    let name = U16String::from_vec(buf);
    assert_eq!("year", name.to_string().unwrap());
}

#[test]
fn mssql_prices() {
    let _ = init().lock();
    let env = unsafe { Environment::new().unwrap() };

    let mut conn = env.connect_with_connection_string(MSSQL).unwrap();
    let sql = "SELECT id,day,time,product,price FROM Sales ORDER BY id;";
    let mut cursor = conn.exec_direct(sql).unwrap().unwrap();

    // Test names
    let mut buf = Vec::new();

    let mut name = |column_number| {
        cursor.col_name(column_number, &mut buf).unwrap();
        std::char::decode_utf16(buf.iter().copied())
            .collect::<Result<String, _>>()
            .unwrap()
    };

    assert_eq!("id", name(1));
    assert_eq!("day", name(2));
    assert_eq!("time", name(3));
    assert_eq!("product", name(4));
    assert_eq!("price", name(5));

    // Test binding id int buffer
    let batch_size = 10;
    assert_eq!(SqlDataType::INTEGER, cursor.col_concise_type(1).unwrap());
    let mut id_buffer: Vec<Integer> = vec![0; batch_size];
    unsafe {
        cursor
            .set_row_array_size(batch_size.try_into().unwrap())
            .unwrap();

        // Bind id integer column
        cursor.bind_col(1, id_buffer.bind_arguments()).unwrap();
        let mut num_rows_fetched = 0;
        cursor.set_num_rows_fetched(&mut num_rows_fetched).unwrap();

        cursor.fetch().unwrap();

        assert_eq!([1, 2, 3], id_buffer[0..num_rows_fetched as usize]);
    }

    cursor.fetch().unwrap();

    // Test types

    assert_eq!(SqlDataType::DECIMAL, cursor.col_concise_type(5).unwrap());
    assert_eq!(10, cursor.col_precision(5).unwrap());
    assert_eq!(2, cursor.col_scale(5).unwrap());
}