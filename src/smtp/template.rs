use schema::devlog::entities::User;

#[derive(Debug, Clone)]
pub enum EmailTemplate {
    WelcomeNewMember(User)
}
