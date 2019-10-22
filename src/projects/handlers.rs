use diesel::prelude::*;
use diesel::{delete, insert_into, update};
use rocket::http::Status;
use rocket::request::Form;
use rocket::response::Redirect;

use rocket_contrib::json::Json;
use serde_json;

use crate::guards::*;
use crate::ObservDbConn;

use super::models::*;
use super::templates::*;

#[get("/projects/<n>")]
pub fn project(conn: ObservDbConn, l: MaybeLoggedIn, n: i32) -> Option<ProjectTemplate> {
    use crate::schema::projects::dsl::*;

    let p: Project = projects
        .find(n)
        .first(&*conn)
        .optional()
        .expect("Failed to get project from database")?;

    Some(ProjectTemplate {
        logged_in: l.user(),
        repos: project_repos(&p),
        users: project_users(&*conn, &p),
        project: p,
    })
}

#[get("/projects/<n>", rank = 2)]
pub fn project_by_handle(conn: ObservDbConn, _l: MaybeLoggedIn, n: String) -> Option<Redirect> {
    use crate::schema::projects::dsl::*;
    let p: Project = projects
        .filter(name.like(n))
        .first(&*conn)
        .optional()
        .expect("Failed to get project from database")?;

    Some(Redirect::to(format!("/projects/{}", p.id)))
}

#[get("/projects/new")]
pub fn project_new(l: UserGuard) -> NewProjectTemplate {
    NewProjectTemplate {
        logged_in: Some(l.0),
    }
}

#[post("/projects/new", data = "<newproject>")]
pub fn project_new_post(
    conn: ObservDbConn,
    l: UserGuard,
    newproject: Form<NewProject>,
) -> Redirect {
    let mut newproject = newproject.into_inner();
    newproject.owner_id = l.0.id;

    newproject.repos = serde_json::to_string(
        &serde_json::from_str::<Vec<String>>(&newproject.repos)
            .unwrap()
            .iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<&String>>(),
    )
    .unwrap();

    use crate::schema::projects::dsl::*;
    insert_into(projects)
        .values(&newproject)
        .execute(&*conn)
        .expect("Failed to insert project into database");

    let p: Project = projects
        .filter(name.eq(newproject.name))
        .first(&*conn)
        .expect("Failed to get project from database");

    use crate::schema::relation_project_user::dsl::*;
    insert_into(relation_project_user)
        .values(&NewRelationProjectUser {
            project_id: p.id,
            user_id: l.0.id,
        })
        .execute(&*conn)
        .expect("Failed to add user to project");

    Redirect::to(format!("/projects/{}", p.id))
}

#[get("/projects/<h>/edit")]
pub fn project_edit(
    conn: ObservDbConn,
    l: UserGuard,
    h: i32,
) -> Result<EditProjectTemplate, Status> {
    use crate::schema::projects::dsl::*;
    use crate::schema::users::dsl::*;

    let p: Project = projects
        .find(h)
        .first(&*conn)
        .expect("Failed to get project from database");
    if l.0.tier > 1 || p.owner_id == l.0.id {
        Ok(EditProjectTemplate {
            logged_in: Some(l.0),
            repos: project_repos(&p),
            project: p,
            all_users: users
                .load(&*conn)
                .expect("Failed to get users from database"),
        })
    } else {
        Err(Status::Unauthorized)
    }
}

#[put("/projects/<h>", data = "<editproject>")]
pub fn project_edit_put(
    conn: ObservDbConn,
    l: UserGuard,
    h: i32,
    editproject: Form<NewProject>,
) -> Result<Redirect, Status> {
    use crate::schema::projects::dsl::*;

    let mut editproject = editproject.into_inner();
    editproject.repos = serde_json::to_string(
        &serde_json::from_str::<Vec<String>>(&editproject.repos)
            .unwrap()
            .iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<&String>>(),
    )
    .unwrap();

    let p: Project = projects
        .find(h)
        .first(&*conn)
        .expect("Failed to get project from database");

    if l.0.tier > 1 || p.owner_id == l.0.id {
        update(projects.find(h))
            .set(&editproject)
            .execute(&*conn)
            .expect("Failed to update project in database");
        Ok(Redirect::to(format!("/projects/{}", h)))
    } else {
        Err(Status::Unauthorized)
    }
}

#[delete("/projects/<h>")]
pub fn project_delete(conn: ObservDbConn, l: UserGuard, h: i32) -> Result<Redirect, Status> {
    use crate::schema::projects::dsl::*;
    let p: Project = projects
        .find(h)
        .first(&*conn)
        .expect("Failed to get project from database");

    if l.0.tier > 1 || p.owner_id == l.0.id {
        use crate::schema::relation_project_user::dsl::*;
        delete(relation_project_user.filter(project_id.eq(h)))
            .execute(&*conn)
            .expect("Failed to delete relations from database");
        use crate::schema::projects::dsl::*;
        delete(projects.find(h))
            .execute(&*conn)
            .expect("Failed to delete project from database");
        Ok(Redirect::to("/projects"))
    } else {
        Err(Status::Unauthorized)
    }
}

#[get("/projects?<s>")]
pub fn projects(conn: ObservDbConn, l: MaybeLoggedIn, s: Option<String>) -> ProjectsListTemplate {
    ProjectsListTemplate {
        logged_in: l.user(),
        projects: filter_projects(&*conn, s),
    }
}

#[get("/projects.json?<s>")]
pub fn projects_json(conn: ObservDbConn, s: Option<String>) -> Json<Vec<Project>> {
    Json(filter_projects(&*conn, s))
}

#[get("/projects/<h>/members")]
pub fn project_members(h: i32) -> Redirect {
    Redirect::to(format!("/projects/{}", h))
}

#[get("/projects/<h>/members.json")]
pub fn project_members_json(conn: ObservDbConn, h: i32) -> Json<Vec<User>> {
    Json(project_users(&*conn, &{
        use crate::schema::projects::dsl::*;
        projects
            .find(h)
            .first(&*conn)
            .expect("Failed to get project from database")
    }))
}

#[get("/projects/<h>/members/add")]
pub fn project_member_add(
    conn: ObservDbConn,
    l: UserGuard,
    h: i32,
) -> Result<AddUserTemplate, Status> {
    let p: Project = {
        use crate::schema::projects::dsl::*;
        projects
            .find(h)
            .first(&*conn)
            .expect("Failed to get project from database")
    };

    let pu = project_users(&*conn, &p);

    use crate::schema::users::dsl::*;

    if l.0.tier > 0 || l.0.id == p.owner_id {
        Ok(AddUserTemplate {
            logged_in: Some(l.0),
            project: p,
            all_users: {
                users
                    .load(&*conn)
                    .expect("Failed to get users from database")
                    .iter()
                    .filter(|&e| !pu.contains(e))
                    .cloned()
                    .collect()
            },
        })
    } else {
        Err(Status::Unauthorized)
    }
}

#[derive(FromForm)]
pub struct UserId {
    pub uid: i32,
}

#[post("/projects/<h>/members/add", data = "<userid>")]
pub fn project_member_add_post(
    conn: ObservDbConn,
    l: UserGuard,
    h: i32,
    userid: Form<UserId>,
) -> Result<Redirect, Status> {
    let p: Project = {
        use crate::schema::projects::dsl::*;
        projects
            .find(h)
            .first(&*conn)
            .expect("Failed to get project from database")
    };

    if l.0.tier > 0 || l.0.id == p.owner_id {
        use crate::schema::relation_project_user::dsl::*;
        insert_into(relation_project_user)
            .values(&NewRelationProjectUser {
                project_id: h,
                user_id: userid.into_inner().uid,
            })
            .execute(&*conn)
            .expect("Failed to insert relation into database");
        Ok(Redirect::to(format!("/projects/{}", h)))
    } else {
        Err(Status::Unauthorized)
    }
}

#[delete("/projects/<h>/members/<uid>")]
pub fn project_member_delete(
    conn: ObservDbConn,
    l: UserGuard,
    h: i32,
    uid: i32,
) -> Result<Redirect, Status> {
    let owner_id: i32 = {
        use crate::schema::projects::dsl::*;
        projects
            .find(h)
            .select(owner_id)
            .first(&*conn)
            .expect("Failed to get project from database")
    };

    if l.0.tier > 0 || l.0.id == owner_id {
        use crate::schema::relation_project_user::dsl::*;
        delete(relation_project_user.filter(project_id.eq(h).and(user_id.eq(uid))))
            .execute(&*conn)
            .expect("Failed to delete relation from database");
        Ok(Redirect::to(format!("/projects/{}", h)))
    } else {
        Err(Status::Unauthorized)
    }
}

#[get("/projects/<h>/members/join")]
pub fn project_join(conn: ObservDbConn, l: UserGuard, h: i32) -> JoinTemplate {
    use crate::schema::projects::dsl::*;
    JoinTemplate {
        logged_in: Some(l.0),
        project: projects
            .find(h)
            .first(&*conn)
            .expect("Failed to get project from database"),
    }
}

#[post("/projects/<h>/members/join")]
pub fn project_join_post(conn: ObservDbConn, l: UserGuard, h: i32) -> Result<Redirect, Status> {
    use crate::schema::projects::dsl::*;

    let a: bool = projects
        .select(active)
        .find(h)
        .first(&*conn)
        .expect("Failed to get project from database");

    if a {
        use crate::schema::relation_project_user::dsl::*;
        insert_into(relation_project_user)
            .values(&NewRelationProjectUser {
                project_id: h,
                user_id: l.0.id,
            })
            .execute(&*conn)
            .expect("Failed to add relation to database");
        Ok(Redirect::to(format!("/projects/{}", h)))
    } else {
        Err(Status::Conflict)
    }
}

pub fn project_repos(p: &Project) -> Vec<String> {
    serde_json::from_str(&p.repos).unwrap()
}

pub fn filter_projects(conn: &SqliteConnection, term: Option<String>) -> Vec<Project> {
    use crate::schema::projects::dsl::*;

    if let Some(term) = term {
        let sterm = format!("%{}%", term);
        let filter = name.like(&sterm);
        projects.filter(filter).load(conn)
    } else {
        projects.load(conn)
    }
    .expect("Failed to get projects")
}

use crate::models::User;

pub fn project_users(conn: &SqliteConnection, project: &Project) -> Vec<User> {
    RelationProjectUser::belonging_to(project)
        .load::<RelationProjectUser>(conn)
        .expect("Failed to get relations from database")
        .iter()
        .map(|r| {
            use crate::schema::users::dsl::*;
            users
                .find(r.user_id)
                .first(conn)
                .expect("Failed to get user from database")
        })
        .collect()
}

/// Get the commits in the project
///
/// This function calls to the GitHub API to get the commits.
///
/// If the project does not use GitHub for it's repo this returns `None`.
/// Otherwise it returns a vector of JSON values with each repo having an
/// entry.
///
/// TODO support other services like GitLab.
pub fn project_commits(conn: &SqliteConnection, proj: &Project) -> Option<Vec<serde_json::Value>> {
    // Get the repos from the DB
    let mut repos: Vec<String> = {
        use crate::schema::projects::dsl::*;
        serde_json::from_str(
            &projects
                .find(proj.id)
                .select(repos)
                .first::<String>(conn)
                .expect("Failed to get repos from the database"),
        )
        .unwrap()
    };

    // No repos at all
    if repos.is_empty() {
        return None;
    }

    // Use a regex to filter to only GitHub and convert to the API string
    use regex::Regex;
    let re = Regex::new(r"^(https?://)?github\.com/(\S+/\S+)/?$")
        .expect("Failed to build regular expression");
    repos = repos
        .iter()
        .filter(|s| re.is_match(&s))
        .map(|s| {
            String::from(re.replace(s, "https://api.github.com/repos/$2/commits?per_page=100"))
        })
        .collect();

    // If no GitHub repos
    if repos.is_empty() {
        return None;
    }

    // Get the commits and return them
    Some(
        repos
            .iter()
            .map(|s| {
                reqwest::get(s)
                    .expect("Failed to get response from GitHub")
                    .json::<serde_json::Value>()
                    .expect("Failed to parse from JSON")
            })
            .collect(),
    )
}
