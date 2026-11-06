//! An example domain: the shape to copy for your own, and the one to delete when you have it.
//!
//! It shows a tenant-scoped resource, a form, a plan limit checked in the write's transaction, and
//! a dashboard tile. Lowering a plan never deletes: a team over its limit keeps its projects and
//! cannot start another.

use jiff::Timestamp;
use tablo::{TenantId, prelude::*, write_create};
use topcoat::{context::Cx, icon::iconify::iconify_icon};

use crate::teams::{current_entitlements, current_role, may_manage_members};

/// Something a team is working on.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
pub struct Project {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[index]
    pub team_id: TenantId,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
    #[update(Timestamp::now())]
    pub updated_at: Timestamp,
}

/// Where a project stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, toasty::Embed, tablo::Options)]
pub enum ProjectStatus {
    #[option(color = "info")]
    Active,
    #[option(color = "warning")]
    Paused,
    #[option(color = "success")]
    Done,
}

/// A project's form.
#[derive(tablo::RecordForm)]
#[form(model = Project)]
pub struct ProjectForm {
    pub name: String,
    pub description: Option<String>,
    #[form(options)]
    pub status: ProjectStatus,
}

/// The team's projects.
pub struct ProjectResource;

impl Resource for ProjectResource {
    type Model = Project;
    type Form = ProjectForm;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .icon(iconify_icon!("lucide:folder"))
            .tenancy(Tenancy::column(lens!(Project.team_id)))
            .record_title(lens!(Project.name))
            .policy(
                Abilities::<Project>::new()
                    .view_any(|cx| current_role(cx).is_some())
                    .view(|cx, _| current_role(cx).is_some())
                    .create(|cx| current_role(cx).is_some())
                    .update(|cx, _| current_role(cx).is_some())
                    .delete(|cx, _| may_manage_members(cx)),
            )
            .table(
                Table::new((
                    TextColumn::new(lens!(Project.name)).searchable().sortable(),
                    TextColumn::new(lens!(Project.status)).badge(),
                    DateColumn::new(lens!(Project.created_at)).sortable(),
                ))
                .option_tabs(lens!(Project.status))
                .default_sort_desc(lens!(Project.created_at)),
            )
    }

    /// Refuses a project past the plan's limit, counting inside the write's transaction.
    async fn create_record(
        cx: &Cx,
        form: ProjectForm,
        ex: &mut dyn toasty::Executor,
    ) -> Result<Project, WriteError<ProjectFormField>> {
        let team = tablo::require_tenant(cx)?;
        let projects = Project::filter(Project::fields().team_id().eq(team))
            .exec(&mut *ex)
            .await?
            .len();
        if !current_entitlements(cx).may_add_project(projects) {
            return Err(WriteError::declined(tablo::text!("error-project-limit")));
        }
        write_create::<Self>(cx, form, ex).await
    }
}
