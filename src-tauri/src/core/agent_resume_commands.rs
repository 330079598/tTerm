use super::agent_resume::{
    BuildAgentResumeCommandInput, ResumeAgent, build_agent_resume_command_service,
};

#[tauri::command]
pub fn build_agent_resume_command(
    agent: ResumeAgent,
    session_id: String,
    cwd: String,
    shell: String,
) -> Option<String> {
    build_agent_resume_command_service(BuildAgentResumeCommandInput {
        agent,
        session_id,
        cwd,
        shell,
    })
}
