pub mod introspection;
pub mod middleware;

/// A subject whose bearer token has been verified by Keycloak introspection.
///
/// Handlers only ever see this type — never a raw token — so there is no way to
/// reach a handler without having passed introspection first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub subject: String,
    pub realm_roles: Vec<String>,
}

impl AuthenticatedUser {
    pub fn has_role(&self, role: &str) -> bool {
        self.realm_roles.iter().any(|r| r == role)
    }
}
