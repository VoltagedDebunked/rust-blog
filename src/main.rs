use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, Arc};
use std::collections::HashMap;
use tokio;

#[derive(Serialize, Clone)]
struct Post {
    id: u32,
    title: String,
    body: String,
}

#[derive(Serialize, Clone)]
struct Comment {
    id: u32,
    post_id: u32,
    text: String,
}

#[derive(Deserialize)]
struct PostData {
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct CommentData {
    post_id: u32,
    text: String,
}

async fn index() -> impl Responder {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Blog Application</title>
        <script src="https://cdn.tailwindcss.com"></script>
    </head>
    <body class="text-gray-900">
        <div class="container mx-auto p-4">
            <h1 class="text-4xl font-bold mb-4">Blog Posts</h1>
            <div id="posts-list" class="space-y-4">Loading...</div>
            <h2 class="text-2xl font-semibold mt-8 mb-2">Create New Post</h2>
            <div class="space-y-2">
                <input id="title" type="text" placeholder="Title" class="p-2 border border-gray-300 rounded w-full"/>
                <textarea id="body" placeholder="Body" class="p-2 border border-gray-300 rounded w-full h-40"></textarea>
                <button onclick="createPost()" class="px-4 py-2 bg-black text-white rounded">Create Post</button>
            </div>
        </div>
        <script>
            async function fetchPosts() {
                let response = await fetch('/api/posts');
                let posts = await response.json();
                let postsList = document.getElementById('posts-list');
                postsList.innerHTML = posts.map(post => `
                    <div class="p-4 bg-white border border-black rounded cursor-pointer" onclick="viewPost(${post.id})">
                        <h2 class="text-2xl font-bold">${post.title}</h2>
                        <p class="mt-2">${post.body}</p>
                    </div>
                `).join('');
            }

            async function viewPost(postId) {
                let response = await fetch(`/api/posts/${postId}`);
                let post = await response.json();
                let commentsResponse = await fetch(`/api/posts/${postId}/comments`);
                let comments = await commentsResponse.json();
                document.body.innerHTML = `
                    <div class="container mx-auto p-4">
                        <h1 class="text-4xl font-bold mb-4">${post.title}</h1>
                        <p class="text-lg mb-4">${post.body}</p>
                        <h2 class="text-2xl font-semibold mb-2">Comments</h2>
                        <div id="comments-list" class="space-y-4">${comments.map(comment => `
                            <div class="p-4 bg-white border border-black rounded">
                                <p>${comment.text}</p>
                            </div>
                        `).join('')}</div>
                        <h2 class="text-2xl font-semibold mt-8 mb-2">Add Comment</h2>
                        <textarea id="comment-text" placeholder="Your comment" class="p-2 border border-gray-300 rounded w-full h-40"></textarea>
                        <button onclick="addComment(${postId})" class="px-4 py-2 bg-black text-white rounded">Add Comment</button>
                    </div>
                `;
            }

            async function createPost() {
                let title = document.getElementById('title').value;
                let body = document.getElementById('body').value;
                let response = await fetch('/api/posts', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({ title, body })
                });
                if (response.ok) {
                    fetchPosts();
                }
            }

            async function addComment(postId) {
                let text = document.getElementById('comment-text').value;
                let response = await fetch('/api/comments', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify({ post_id: postId, text })
                });
                if (response.ok) {
                    viewPost(postId);
                }
            }

            window.onload = fetchPosts;
        </script>
    </body>
    </html>
    "#;
    HttpResponse::Ok().content_type("text/html").body(html)
}

async fn get_posts(db: web::Data<Arc<Mutex<HashMap<u32, Post>>>>) -> impl Responder {
    let db = db.lock().unwrap();
    let posts: Vec<Post> = db.values().cloned().collect();
    HttpResponse::Ok().json(posts)
}

async fn create_post(post: web::Json<PostData>, db: web::Data<Arc<Mutex<HashMap<u32, Post>>>>) -> impl Responder {
    let mut db = db.lock().unwrap();
    let id = (db.len() as u32) + 1;
    let new_post = Post {
        id,
        title: post.title.clone(),
        body: post.body.clone(),
    };
    db.insert(id, new_post);
    HttpResponse::Created().finish()
}

async fn get_post(post_id: web::Path<u32>, db: web::Data<Arc<Mutex<HashMap<u32, Post>>>>) -> impl Responder {
    let db = db.lock().unwrap();
    if let Some(post) = db.get(&post_id.into_inner()) {
        HttpResponse::Ok().json(post.clone())
    } else {
        HttpResponse::NotFound().finish()
    }
}

async fn get_comments(post_id: web::Path<u32>, comments_db: web::Data<Arc<Mutex<HashMap<u32, Vec<Comment>>>>>) -> impl Responder {
    let comments_db = comments_db.lock().unwrap();
    if let Some(comments) = comments_db.get(&post_id.into_inner()) {
        HttpResponse::Ok().json(comments.clone())
    } else {
        HttpResponse::Ok().json(Vec::<Comment>::new())
    }
}

async fn create_comment(comment: web::Json<CommentData>, comments_db: web::Data<Arc<Mutex<HashMap<u32, Vec<Comment>>>>>) -> impl Responder {
    let mut comments_db = comments_db.lock().unwrap();
    let post_id = comment.post_id;
    let new_comment = Comment {
        id: comments_db.get(&post_id).map_or(1, |comments| comments.len() as u32 + 1),
        post_id,
        text: comment.text.clone(),
    };
    comments_db.entry(post_id)
        .or_insert_with(Vec::new)
        .push(new_comment);
    HttpResponse::Created().finish()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let posts_db: Arc<Mutex<HashMap<u32, Post>>> = Arc::new(Mutex::new(HashMap::new()));
    let comments_db: Arc<Mutex<HashMap<u32, Vec<Comment>>>> = Arc::new(Mutex::new(HashMap::new()));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(posts_db.clone()))
            .app_data(web::Data::new(comments_db.clone()))
            .route("/", web::get().to(index))
            .route("/api/posts", web::get().to(get_posts))
            .route("/api/posts", web::post().to(create_post))
            .route("/api/posts/{id}", web::get().to(get_post))
            .route("/api/posts/{id}/comments", web::get().to(get_comments))
            .route("/api/comments", web::post().to(create_comment))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}