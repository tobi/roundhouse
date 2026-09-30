# The ruby lane's config.ru, with a backtrace logged for any request that
# raises — what scripts/campfire-db-differential prints when the
# scenario fails part way, so the failing statement is named.
class TraceErrors
  def initialize(app) = @app = app

  def call(env)
    @app.call(env)
  rescue Exception => e
    warn "TRACE #{e.class}: #{e.message}\n" + e.backtrace.first(8).join("\n")
    raise
  end
end
use TraceErrors
eval File.read("config.ru"), binding, "config.ru"
