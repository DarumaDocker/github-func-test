use flowsnet_platform_sdk::logger;
use std::collections::HashMap;

use http_req::request;
use openai_flows::{
    chat::{self, ChatModel, ChatOptions, ResponseFormat, ResponseFormatType},
    OpenAIFlows,
};
use sendgrid::v3::*;

use serde::Deserialize;
use serde_json::Value;
use webhook_flows::{
    create_endpoint, request_handler,
    route::{get, options, post, route, RouteError, Router},
    send_response,
};

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn on_deploy() {
    create_endpoint().await;
}

#[request_handler]
async fn handler() {
    let mut router = Router::new();
    router.insert("/options", vec![options(opt)]).unwrap();
    router
        .insert("/get/:city", vec![options(opt), get(query)])
        .unwrap();
    router.insert("/openai", vec![post(openai)]).unwrap();
    router.insert("/email", vec![post(email)]).unwrap();
    router.insert("/ggml", vec![post(ggml)]).unwrap();

    if let Err(e) = route(router).await {
        match e {
            RouteError::NotFound => {
                send_response(404, vec![], b"No route matched".to_vec());
            }
            RouteError::MethodNotAllowed => {
                send_response(405, vec![], b"Method not allowed".to_vec());
            }
        }
    }
}
async fn ggml(_headers: Vec<(String, String)>, _qry: HashMap<String, Value>, body: Vec<u8>) {
    logger::init();

    let input = String::from_utf8_lossy(&body).into_owned();

    let result =
        wasi_nn::GraphBuilder::new(wasi_nn::GraphEncoding::Ggml, wasi_nn::ExecutionTarget::CPU)
            .build_from_cache("default");
    let graph = match result {
        Ok(graph) => graph,
        Err(err) => {
            println!("Failed to build graph: {:?}", err);
            return;
        }
    };

    let mut context = graph.init_execution_context().unwrap();

    let system_prompt = String::from("<<SYS>>You are a helpful, respectful and honest assistant. Always answer as short as possible, while being safe. <</SYS>>");

    let prompt = format!(
        "<s>[INST] <<SYS>>\n{system_prompt}\n<</SYS>>\n\n{user_message} [/INST]",
        system_prompt = system_prompt.trim(),
        user_message = input.trim()
    );

    let tensor_data = prompt.trim().as_bytes().to_vec();
    context
        .set_input(0, wasi_nn::TensorType::U8, &[1], &tensor_data)
        .unwrap();

    // Execute the inference.
    context.compute().unwrap();

    // Retrieve the output.
    let mut output_buffer = vec![0u8; 2048];
    let output_size = context.get_output(0, &mut output_buffer).unwrap();

    send_response(200, vec![], output_buffer[..output_size].to_vec());
}

async fn email(_headers: Vec<(String, String)>, qry: HashMap<String, Value>, body: Vec<u8>) {
    logger::init();

    let receiver = qry.get("receiver").unwrap().as_str().unwrap();
    let subject = qry.get("subject").unwrap().as_str().unwrap();

    let sender = std::env::var("SENDGRID_SENDER").unwrap();
    let sg_api_key = std::env::var("SENDGRID_API_KEY").unwrap();

    let mut cool_header = HashMap::with_capacity(2);
    cool_header.insert(String::from("x-cool"), String::from("indeed"));
    cool_header.insert(String::from("x-cooler"), String::from("cold"));

    let p = Personalization::new(Email::new(receiver)).add_headers(cool_header);

    let m = Message::new(Email::new(sender))
        .set_subject(subject)
        .add_content(
            Content::new()
                .set_content_type("text/html")
                .set_value(String::from_utf8_lossy(&body)),
        )
        .add_personalization(p);

    let sender = Sender::new(sg_api_key);
    match sender.send(&m).await {
        Ok(resp) => {
            send_response(200, vec![], format!("{resp:#?}").into_bytes().to_vec());
        }
        Err(e) => {
            send_response(500, vec![], format!("{e:#?}").into_bytes().to_vec());
        }
    }
}

async fn openai(_headers: Vec<(String, String)>, _qry: HashMap<String, Value>, body: Vec<u8>) {
    let msg = String::from_utf8_lossy(&body).into_owned();

    let of = OpenAIFlows::new();
    let co = ChatOptions {
        model: ChatModel::GPT4Turbo,
        max_tokens: Some(500),
        response_format: Some(ResponseFormat {
            r#type: ResponseFormatType::JsonObject,
        }),
        ..chat::ChatOptions::default()
    };

    match of.chat_completion("test", &msg, &co).await {
        Ok(c) => send_response(200, vec![], c.choice.into_bytes().to_vec()),
        Err(e) => send_response(500, vec![], e.into_bytes().to_vec()),
    }
}

// #[request_handler(OPTIONS)]
async fn opt(
    _headers: Vec<(String, String)>,
    // _subpath: String,
    _qry: HashMap<String, Value>,
    _body: Vec<u8>,
) {
    send_response(
        200,
        vec![
            (
                String::from("Allow"),
                String::from("OPTIONS, GET, HEAD, POST"),
            ),
            (
                String::from("Access-Control-Allow-Origin"),
                String::from("http://127.0.0.1"),
            ),
            (
                String::from("Access-Control-Allow-Methods"),
                String::from("POST, GET, OPTIONS"),
            ),
        ],
        vec![],
    );
}

// #[request_handler(GET, POST)]
async fn query(
    _headers: Vec<(String, String)>,
    // _subpath: String,
    qry: HashMap<String, Value>,
    _body: Vec<u8>,
) {
    flowsnet_platform_sdk::logger::init();

    let city = qry.get("city").unwrap_or(&Value::Null).as_str();
    let resp = match city {
        Some(c) => get_weather(c).map(|w| {
            format!(
                "Today: {},
Low temperature: {} °C,
High temperature: {} °C,
Wind Speed: {} km/h",
                w.weather
                    .first()
                    .unwrap_or(&Weather {
                        main: "Unknown".to_string()
                    })
                    .main,
                w.main.temp_min as i32,
                w.main.temp_max as i32,
                w.wind.speed as i32
            )
        }),
        None => Err(String::from("No city in query")),
    };

    match resp {
        Ok(r) => send_response(
            200,
            vec![(
                String::from("content-type"),
                String::from("text/html; charset=UTF-8"),
            )],
            r.as_bytes().to_vec(),
        ),
        Err(e) => {
            send_response(
                400,
                vec![(
                    String::from("content-type"),
                    String::from("text/html; charset=UTF-8"),
                )],
                e.as_bytes().to_vec(),
            );
        }
    }
}

#[derive(Deserialize)]
struct ApiResult {
    weather: Vec<Weather>,
    main: Main,
    wind: Wind,
}

#[derive(Deserialize)]
struct Weather {
    main: String,
}

#[derive(Deserialize)]
struct Main {
    temp_max: f64,
    temp_min: f64,
}

#[derive(Deserialize)]
struct Wind {
    speed: f64,
}

fn get_weather(city: &str) -> Result<ApiResult, String> {
    let mut writer = Vec::new();
    let api_key = "09a55b004ce2f065b903015e3284de35";
    let query_str = format!(
        "https://api.openweathermap.org/data/2.5/weather?q={city}&units=metric&appid={api_key}"
    );

    request::get(query_str, &mut writer)
        .map_err(|e| e.to_string())
        .and_then(|_| {
            serde_json::from_slice::<ApiResult>(&writer).map_err(|_| {
                "Please check if you've typed the name of your city correctly".to_string()
            })
        })
}
