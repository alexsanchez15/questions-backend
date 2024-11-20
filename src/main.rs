//i need to make a backend to a webserver.
//this will handle all of the server side data,
//this includes:
//code logic    database interaction    server management
//handles requests sent from the frontend, managing responses
//those responses can be something like fetching or storing data,
//or any other logic that does not make sense to preform client side.

use actix_cors::Cors;
use actix_files::{Files, NamedFile};
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use serde::Serialize;
use sqlx::PgPool;
use sqlx::{postgres::PgPoolOptions, Row};
use std::env;
use std::io::Result;

//fallback onto these for dynamic address
async fn fallback(req: HttpRequest) -> impl Responder {
    // Try to open index.html, which will be served if no other route matches
    let index_path = "./build/index.html";
    match actix_files::NamedFile::open(index_path) {
        Ok(file) => file.into_response(&req),        // Serve index.html
        Err(_) => HttpResponse::NotFound().finish(), // If not found, return 404
    }
}

#[post("/api/")]
async fn home_page(data: web::Json<String>, pool: web::Data<PgPool>) -> impl Responder {
    let table = data.into_inner();
    let exists = does_table_exist(&pool, &table).await;
    if exists {
        return HttpResponse::Ok().body("success");
    } else {
        //WE HAVE table questions_test (id SERIAL PRIMARY KEY, question VARCHAR(255), votes INT DEFAULT 0);
        let query = format!(
            "
            CREATE TABLE {} (id SERIAL PRIMARY KEY, question VARCHAR(255), votes INT DEFAULT 0)",
            table
        );
        let result = sqlx::query(&query).execute(pool.as_ref()).await;
        match result {
            Ok(_) => {
                return HttpResponse::Ok().body("success");
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("{}", e)),
        }
    }
}

async fn does_table_exist(pool: &PgPool, path: &str) -> bool {
    let query = "
        SELECT EXISTS (
            SELECT 1
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_SCHEMA = 'public'
            AND TABLE_NAME = $1
        );
    ";
    let row = sqlx::query(query).bind(path).fetch_one(pool).await;
    let rowresult = row.expect("error unwrapping row in dynamic get address.");
    let exists: bool = rowresult.get(0);
    return exists;
}

#[get("/api/{path}")]
async fn dynamic_get(path: web::Path<String>, pool: web::Data<PgPool>) -> impl Responder {
    // check if table exists for this website or not. And if not, return error.
    let path = path.to_string();
    let exists = does_table_exist(&pool, &path).await;

    if exists {
        //passing &path to create questions list bc the path is the table name.
        let questions = create_questions_list_json(&pool, &path).await;
        match questions {
            Ok(q) => HttpResponse::Ok().json(q), //return the questions
            Err(e) => {
                HttpResponse::InternalServerError().body(format!("Error fetching questions: {}", e))
            }
        }
    } else {
        HttpResponse::InternalServerError().body(format!("ERROR! NO SITE EXISTS"))
    }
}
//our get method returns the questions as json object
//now we want our post method to recieve some information

#[derive(Deserialize)]
struct QuestionPayload {
    action: String,
    question: Option<String>, //string of question (used when entering new votes)
    question_id: Option<i32>, //id of the question (used with voting)
    vote_type: Option<String>, //upvote, downvote
}
#[post("/api/{path}")]
async fn alex_post(
    payload: web::Json<QuestionPayload>,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> impl Responder {
    //write code to accept what is either an upvote or a question, and handle it appropriately.
    /*
     * Format of JSON will be like this
     * { action: string --- either vote or question
     *  { question: "quetsion.
     *  { quetsion_id : id for voting
     *  { vote_type : also for voting
     *  they are all options.
     */
    let path = path.to_string();
    match payload.action.as_str() {
        "create" => {
            //for creating a new question
            let question = payload.question.clone().unwrap();
            let query = format!("INSERT INTO {} (question) VALUES ($1)", path); // Use direct string for the query
            match sqlx::query(&query)
                .bind(&question) // Bind the question variable
                .execute(pool.as_ref()) // get the refrenece
                .await
            {
                Ok(_) => HttpResponse::Ok().body("Question created."),
                Err(e) => {
                    eprintln!("Failed to insert question: {}", e); // Log error for debugging
                    HttpResponse::InternalServerError().body("Error creating question.")
                }
            }
        }
        "vote" => {
            let qid = payload.question_id.clone().unwrap(); //unsafe but assuming things are written fine
            let vote_type = payload.vote_type.clone().unwrap(); //unsafe; either upvote or downvote
            println!(
                "Received vote for question ID: {}, type: {}",
                qid, vote_type
            );

            let query = if vote_type == "upvote" {
                format!("UPDATE {} SET votes = votes + 1 WHERE id = $1", path)
            } else if vote_type == "downvote" {
                //This will be a little different. Check if votecount is
                //already -2. if so, as -3 is deletion, this will call a
                //removal.
                //if not, it will simply return an update
                let rowresult = sqlx::query(&format!("SELECT votes FROM {} WHERE id = $1", path))
                    .bind(qid)
                    .fetch_one(pool.as_ref())
                    .await;
                let row = rowresult.expect("error unwrapping row"); //should never have any issues
                let votes = row.get::<i32, _>("votes");
                if votes == -2 {
                    format!("DELETE FROM {} WHERE id = $1", path)
                } else {
                    format!("UPDATE {} SET votes = votes - 1 WHERE id = $1", path)
                }
            } else {
                "null".to_string()
            };
            let _ = sqlx::query(&query).bind(qid).execute(pool.as_ref()).await;
            //dont really have any use for this but will expand error handling here if necessary
            HttpResponse::Ok().body("vote recieved")
        }
        _ => HttpResponse::InternalServerError().body("Bad payload passed"),
    }
}

#[actix_web::main]
async fn main() -> Result<()> {
    //make a pool that the web data will hold
    let database_url = "postgres://alex:password@localhost/test";
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed");

    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                    .allow_any_header(),
            )
            .app_data(web::Data::new(pool.clone()))
            .service(home_page)
            .service(alex_post)
            .service(dynamic_get)
            .service(Files::new("/", "./build").index_file("index.html"))
            .default_service(web::route().to(fallback))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
use serde_json::json;
use std;
async fn create_questions_list_json(
    pool: &PgPool,
    table_name: &str,
) -> std::result::Result<serde_json::Value, sqlx::Error> {
    let query = format!("SELECT id, question, votes FROM {}", table_name);
    let rows = sqlx::query(&query).fetch_all(pool).await?;

    let questions: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.get::<i32, _>("id"),
                "question": row.get::<String, _>("question"),
                "votes": row.get::<i32, _>("votes"),
            })
        })
        .collect();

    Ok(json!(questions))
}
//WE HAVE table questions_test (id SERIAL PRIMARY KEY, question VARCHAR(255), votes INT DEFAULT 0);
//for now not using it though, but will come soon.
