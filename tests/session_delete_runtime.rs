//! The store's post-authentication redirect consumes a session entry.
//! `cli_check::the_store_fixture_checks_clean` pins its diagnostic gate;
//! this test pins the behavior of the emitted controller and session.

#[path = "support/emit_and_run.rs"]
mod emit_and_run;

#[test]
fn deleting_a_session_redirect_returns_the_value_then_falls_back() {
    emit_and_run::real_blog()
        .write(
            "app/controllers/application_controller.rb",
            r#"class ApplicationController < ActionController::Base
  private
    def after_authentication_url
      session.delete(:return_to_after_authenticating) || root_url
    end
end
"#,
        )
        .run_ruby(
            r#"require File.expand_path("app/controllers/application_controller", Dir.pwd)
controller = ApplicationController.new
controller.session[:return_to_after_authenticating] = "/articles/123"
raise "lost stored redirect" unless controller.send(:after_authentication_url) == "/articles/123"
raise "redirect key was not deleted" if controller.session.key?(:return_to_after_authenticating)
fallback = controller.send(:after_authentication_url)
raise "missing redirect did not fall back to root" unless fallback == "/"
raise "missing session key did not return nil" unless controller.session.delete(:missing).nil?
"#,
        )
        .assert_passes();
}
