//i need to make a backend to a webserver.
//this will handle all of the server side data,
//this includes:
//code logic    database interaction    server management
//handles requests sent from the frontend, managing responses
//those responses can be something like fetching or storing data,
//or any other logic that does not make sense to preform client side.

use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use serde::Serialize;
use sqlx::PgPool;
use sqlx::{postgres::PgPoolOptions, Row};
use std::env;
use std::io::Result;

#[get("/")]
async fn home_page() -> impl Responder {
    let database_url = "postgres://alex:password@localhost/test";
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed");
    let rows = sqlx::query("SELECT id, name, email FROM test_table")
        .fetch_all(&pool)
        .await;
    match rows {
        Ok(rows) => {
            let mut all_rows = String::new();
            for row in rows {
                all_rows.push_str(&format!(
                    "ID: {}, Name: {}, Email: {}",
                    row.get::<i32, _>("id"),
                    row.get::<Option<String>, _>("name")
                        .unwrap_or("unknown".to_string()),
                    row.get::<Option<String>, _>("email")
                        .unwrap_or("unknown".to_string()),
                ));
            }
            HttpResponse::Ok().body(all_rows)
        }
        Err(e) => HttpResponse::Ok().body(format!("error reading database: {}", e)),
    }
}
#[get("/{path}")]
async fn dynamic_get(path: web::Path<String>, pool: web::Data<PgPool>) -> impl Responder {
    // check if table exists for this website or not. And if not, return error.
    let path = path.to_string();
    let query = "
        SELECT EXISTS (
            SELECT 1
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_SCHEMA = 'public'
            AND TABLE_NAME = $1
        );
    ";
    let row = sqlx::query(query)
        .bind(path.clone())
        .fetch_one(pool.as_ref())
        .await;
    let rowresult = row.expect("error unwrapping row in dynamic get address.");
    let exists: bool = rowresult.get(0);

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
#[post("/alex")]
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
            let query = "INSERT INTO $1 (question) VALUES ($2)"; // Use direct string for the query
            match sqlx::query(query)
                .bind(path.clone())
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
                "UPDATE $1 SET votes = votes + 1 WHERE id = $2"
            } else if vote_type == "downvote" {
                //This will be a little different. Check if votecount is
                //already -2. if so, as -3 is deletion, this will call a
                //removal.
                //if not, it will simply return an update
                let rowresult = sqlx::query("SELECT votes FROM $1 WHERE id = $2")
                    .bind(path.clone())
                    .bind(qid)
                    .fetch_one(pool.as_ref())
                    .await;
                let row = rowresult.expect("error unwrapping row"); //should never have any issues
                let votes = row.get::<i32, _>("votes");
                if votes == -2 {
                    let _ = sqlx::query("DELETE FROM $1 WHERE id = $2")
                        .bind(path.clone())
                        .bind(qid)
                        .execute(pool.as_ref())
                        .await;
                    return HttpResponse::Ok().body("Post deleted"); //will kill the post
                } else {
                    "UPDATE $1 SET votes = votes - 1 WHERE id = $2"
                }
            } else {
                return HttpResponse::InternalServerError().body("error with voting");
            };
            let _ = sqlx::query(query)
                .bind(path)
                .bind(qid)
                .execute(pool.as_ref())
                .await;
            //dont really have any use for this but will expand error handling here if necessary
            HttpResponse::Ok().body("vote recieved")
        }
        _ => HttpResponse::InternalServerError().body("Bad payload passed"),
    }
}

#[get("/alex")]
async fn alex() -> impl Responder {
    let database_url = "postgres://alex:password@localhost/test";
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed");
    //get the questions and return them
    let questions = create_questions_list_json(&pool, "questions_test").await;
    match questions {
        Ok(q) => HttpResponse::Ok().json(q),
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Error fetching questions: {}", e))
        }
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
            .service(alex)
            .service(alex_post)
            .service(dynamic_get)
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
