mod answers;
mod words;

use rand::Rng;
use rouille::router;
use rouille::Request;
use rouille::Response;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Copy, Clone)]
enum MatchType {
    Perfect,
    Partial,
    None,
}

#[derive(Serialize, Copy, Clone)]
struct CharMatch {
    index: usize,
    character: char,
    match_type: MatchType,
}

struct Game {
    word: String,
    solved: bool,
}

#[derive(Serialize)]
struct GameIdentity {
    game_id: String,
}

#[derive(Serialize)]
struct Answer {
    solved: bool,
    guess: String,
    evaluation: Vec<CharMatch>,
}

fn main() {
    let conn = get_connection();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS game (
            game_id TEXT NOT NULL,
            word    TEXT NOT NULL,
            goes    INTEGER DEFAULT 0,
            solved  INTEGER DEFAULT 0
        )",
        (),
    )
    .unwrap();

    rouille::start_server("0.0.0.0:85", move |request| handle_request(request));
}

fn handle_request(request: &Request) -> Response {
    router!(request,
        (GET) (/play/{game_id: String}/guess/{guess: String}) => { handle_play(&game_id, &guess) },

        (GET) (/create) => { handle_new_game() },

        _ => Response::empty_404()
    )
}

fn handle_play(game_id: &str, guess: &str) -> Response {
    let conn = get_connection();

    let game_result = conn.query_row(
        "SELECT game_id, word, goes, solved FROM game WHERE game_id = ?1",
        [game_id],
        |row| {
            Ok(Game {
                word: row.get_unwrap(1),
                solved: row.get_unwrap(3),
            })
        },
    );

    if let Err(error) = game_result {
        return Response::text(error.to_string()).with_status_code(404);
    }

    let game = game_result.unwrap();
    if game.solved {
        return Response::text("It's already been solved!");
    }

    let words = words::FILE_CONTENT;

    if !words.contains(&guess) {
        return Response::text(format!("'{guess}' is not a valid guess")).with_status_code(400);
    }

    let answer = evaluate_guess(&game.word, &guess);

    conn.execute(
        "UPDATE game SET goes = goes + 1, solved = ?1 WHERE game_id = ?2",
        [if answer.solved { "1" } else { "0" }, game_id],
    )
    .unwrap();

    Response::text(serde_json::to_string_pretty(&answer).unwrap())
}

fn handle_new_game() -> Response {
    let conn = get_connection();
    let game_id: Uuid = Uuid::new_v4();

    let random_answer = random_answer();
    conn.execute(
        "INSERT INTO game (game_id, word, goes) VALUES (?1, ?2, ?3)",
        (&game_id.to_string(), &random_answer, 0),
    )
    .unwrap();

    Response::text(
        serde_json::to_string_pretty(&GameIdentity {
            game_id: game_id.to_string(),
        })
        .unwrap(),
    )
}

fn random_answer() -> String {
    let words = answers::FILE_CONTENT;

    let mut rng = rand::thread_rng();
    let random_index = rng.gen_range(0..words.len());

    words[random_index].to_string()
}

fn get_connection() -> Connection {
    Connection::open("wordle.db").unwrap()
}

fn evaluate_guess(word: &str, guess: &str) -> Answer {
    let mut word_chars = word.chars().collect::<Vec<char>>();
    let mut evaluation = guess
        .chars()
        .clone()
        .enumerate()
        .map(|(index, x)| CharMatch {
            index,
            character: x,
            match_type: MatchType::None,
        })
        .collect::<Vec<CharMatch>>();

    // find the perfect matches
    guess.chars().enumerate().for_each(|(i, guess_char)| {
        if word_chars[i] == guess_char {
            evaluation[i] = CharMatch {
                index: i,
                character: guess_char,
                match_type: MatchType::Perfect,
            };

            // prevent this character being re-used for a match
            word_chars[i] = '_'
        }
    });

    // find the partial matches
    guess.chars().enumerate().for_each(|(i, guess_char)| {
        let word_index_match = word_chars.iter().position(|&x| x == guess_char);

        if let Some(word_index) = word_index_match {
            evaluation[i] = CharMatch {
                index: i,
                character: guess_char,
                match_type: MatchType::Partial,
            };

            // prevent this character being re-used for a match
            word_chars[word_index] = '_'
        }
    });

    Answer {
        solved: word.eq(guess),
        guess: guess.to_string(),
        evaluation,
    }
}
