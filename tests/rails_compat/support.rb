# VENDORED from basecamp/once-campfire-rust (MIT), reference-tools/support.rb at
# 080f9035141ac09e1db81476a1b229ed24f08d8f. Run by OUR oracle (a copy of build/campfire-oracle
# with storage of its own, e.g. the build/campfire-parity tree scripts/campfire-http-shape makes):
#
#   cd build/campfire-parity && RAILS_ENV=production RAILS_LOG_LEVEL=fatal DISABLE_SSL=1 \
#     SKIP_TELEMETRY=true SECRET_KEY_BASE=<secret_key_base from rails_compat.json> \
#     VECTORS_DIR=<out> DATABASE_URL=sqlite3:<out>/ref.sqlite3 \
#     bin/rails runner ../../tests/rails_compat/rails_compat_vectors.rb
#
# Every deterministic section comes out byte-identical to the vendored rails_compat.json
# (checked 2026-09-29); the sections with randomness in them differ in their random parts only.
#
# Shared helpers for the reference-tools scripts. Loaded with `require_relative "support"` from
# scripts run by `reference-tools/run.sh` (bin/rails runner, production env).
require "json"
require "active_support/testing/time_helpers"
require "action_controller/test_case"

module ReferenceTools
  include ActiveSupport::Testing::TimeHelpers

  # A fixed "now" so expiry timestamps in the vectors are reproducible.
  NOW = Time.utc(2026, 1, 1, 12, 0, 0)

  # A second secret that plays the part of a rotated SECRET_KEY_BASE.
  ROTATED_SECRET_KEY_BASE = "0" * 64 + "rotated" + "f" * 57

  USER_AGENT = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
  HOST = "campfire.test"

  def app
    Rails.application
  end

  def vectors_dir
    ENV.fetch("VECTORS_DIR")
  end

  # Runs the block at `time`, then returns to NOW (travel_to can't nest).
  def at(time)
    travel_back
    travel_to(time)
    yield
  ensure
    travel_back
    travel_to(NOW)
  end

  def iso(time)
    time&.utc&.iso8601(3)
  end

  # Recreates the schema in the scratch database and seeds the few records the vectors need.
  def reset_database!
    ActiveRecord::Schema.verbose = false
    silence_stream($stdout) { load Rails.root.join("db/schema.rb") }
    ActiveRecord::Base.connection.execute("DELETE FROM sqlite_sequence") rescue nil

    Account.create!(name: "Campfire")
    @david = User.create!(name: "David", email_address: "david@example.com", password: "secret123456", role: :administrator)
    @jason = User.create!(name: "Jason", email_address: "jason@example.com", password: "secret123456")
    @room = Rooms::Open.create!(name: "All Talk", creator: @david)
  end

  def silence_stream(stream)
    old = stream.dup
    stream.reopen(File::NULL)
    yield
  ensure
    stream.reopen(old)
  end

  # --- Requests --------------------------------------------------------------------------------

  def cookie_header(cookies)
    cookies.map { |name, value| "#{name}=#{Rack::Utils.escape(value)}" }.join("; ")
  end

  def request_for(cookies: {}, env: {}, path: "/")
    rack_env = Rack::MockRequest.env_for("http://#{HOST}#{path}", "HTTP_COOKIE" => cookie_header(cookies))
    ActionDispatch::Request.new(app.env_config.merge(rack_env).merge(env))
  end

  def rotated_env
    { "action_dispatch.key_generator" => rotated_key_generator, "action_dispatch.secret_key_base" => ROTATED_SECRET_KEY_BASE }
  end

  def rotated_key_generator
    ActiveSupport::CachingKeyGenerator.new(ActiveSupport::KeyGenerator.new(ROTATED_SECRET_KEY_BASE, iterations: 1000))
  end

  # Writes a cookie through the real cookie jar and returns [jar value, Set-Cookie header].
  def write_cookie(env: {})
    request = request_for(env: env)
    yield request.cookie_jar
    response = Rack::Response.new
    request.cookie_jar.write(response)
    header = Array(response.headers["set-cookie"]).first
    wire = header[/\A[^=]+=([^;]*)/, 1]
    [ Rack::Utils.unescape(wire), header ]
  end

  def read_cookie(kind, name, raw)
    request_for(cookies: { name => raw }).cookie_jar.public_send(kind)[name]
  end

  # Runs a full request through the app (the whole middleware stack) and returns the response.
  def perform(method, path, cookies: {}, params: nil, headers: {})
    env = {
      method: method.to_s.upcase, "HTTP_HOST" => HOST, "HTTP_USER_AGENT" => USER_AGENT,
      "HTTP_ACCEPT" => "text/html", "REMOTE_ADDR" => "127.0.0.1"
    }
    env["HTTP_COOKIE"] = cookie_header(cookies) if cookies.any?
    env[:params] = params if params
    status, response_headers, body = app.call(Rack::MockRequest.env_for("http://#{HOST}#{path}", env.merge(headers)))
    text = +""
    body.each { |part| text << part }
    body.close if body.respond_to?(:close)
    [ status, response_headers, text ]
  end

  def set_cookies(headers)
    Array(headers["set-cookie"]).flat_map { |h| h.split("\n") }.to_h do |header|
      name, value = header[/\A[^;]*/].split("=", 2)
      [ name, { "wire" => value, "raw" => Rack::Utils.unescape(value), "header" => header } ]
    end
  end

  # --- CSRF --------------------------------------------------------------------------------------

  def csrf_controller(session_token, path: "/", method: "POST")
    request = request_for(path: path, env: { "REQUEST_METHOD" => method })
    request.session = ActionController::TestSession.new("_csrf_token" => session_token)
    ApplicationController.new.tap { |controller| controller.set_request!(request) }
  end
end
