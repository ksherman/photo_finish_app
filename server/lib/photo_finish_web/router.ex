defmodule PhotoFinishWeb.Router do
  use PhotoFinishWeb, :router

  import Oban.Web.Router
  import Oban.Web.Router
  use AshAuthentication.Phoenix.Router

  import AshAuthentication.Plug.Helpers

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {PhotoFinishWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
    plug :load_from_session
  end

  pipeline :api do
    plug :accepts, ["json"]

    plug AshAuthentication.Strategy.ApiKey.Plug,
      resource: PhotoFinish.Accounts.User,
      # if you want to require an api key to be supplied, set `required?` to true
      required?: false

    plug :load_from_bearer
    plug :set_actor, :user
  end

  pipeline :admin_auth do
    plug :basic_auth
  end

  scope "/", PhotoFinishWeb do
    pipe_through :browser

    ash_authentication_live_session :authenticated_routes do
      # in each liveview, add one of the following at the top of the module:
      #
      # If an authenticated user must be present:
      # on_mount {PhotoFinishWeb.LiveUserAuth, :live_user_required}
      #
      # If an authenticated user *may* be present:
      # on_mount {PhotoFinishWeb.LiveUserAuth, :live_user_optional}
      #
      # If an authenticated user must *not* be present:
      # on_mount {PhotoFinishWeb.LiveUserAuth, :live_no_user}
    end
  end

  scope "/", PhotoFinishWeb do
    pipe_through :browser

    get "/", PageController, :home
    auth_routes AuthController, PhotoFinish.Accounts.User, path: "/auth"
    sign_out_route AuthController

    # Remove these if you'd like to use your own authentication views
    sign_in_route register_path: "/register",
                  reset_path: "/reset",
                  auth_routes_prefix: "/auth",
                  on_mount: [{PhotoFinishWeb.LiveUserAuth, :live_no_user}],
                  overrides: [
                    PhotoFinishWeb.AuthOverrides,
                    Elixir.AshAuthentication.Phoenix.Overrides.DaisyUI
                  ]

    # Remove this if you do not want to use the reset password feature
    reset_route auth_routes_prefix: "/auth",
                overrides: [
                  PhotoFinishWeb.AuthOverrides,
                  Elixir.AshAuthentication.Phoenix.Overrides.DaisyUI
                ]

    # Remove this if you do not use the confirmation strategy
    confirm_route PhotoFinish.Accounts.User, :confirm_new_user,
      auth_routes_prefix: "/auth",
      overrides: [
        PhotoFinishWeb.AuthOverrides,
        Elixir.AshAuthentication.Phoenix.Overrides.DaisyUI
      ]

    # Remove this if you do not use the magic link strategy.
    magic_sign_in_route(PhotoFinish.Accounts.User, :magic_link,
      auth_routes_prefix: "/auth",
      overrides: [
        PhotoFinishWeb.AuthOverrides,
        Elixir.AshAuthentication.Phoenix.Overrides.DaisyUI
      ]
    )
  end

  # Public viewer routes (no authentication)
  scope "/viewer", PhotoFinishWeb do
    pipe_through :browser

    # Event picker (lists active events)
    live "/", ViewerLive.Home, :index
    # Event-scoped viewer
    live "/:event_id", ViewerLive.Home, :index
    live "/:event_id/competitor/:id", ViewerLive.Competitor, :show
    live "/:event_id/competitor/:id/order", ViewerLive.Order, :new

    get "/photos/thumbnail/:id", Viewer.PhotoController, :thumbnail
    get "/photos/preview/:id", Viewer.PhotoController, :preview
  end

  scope "/admin", PhotoFinishWeb.Admin do
    pipe_through [:browser, :admin_auth]

    oban_dashboard("/oban")

    live "/events", EventLive.Index, :index
    live "/events/new", EventLive.Form, :new
    live "/events/:id/edit", EventLive.Form, :edit
    live "/events/:id", EventLive.Show, :show
    live "/events/:id/show/edit", EventLive.Show, :edit

    # Competitor roster import
    live "/events/:event_id/import-roster", CompetitorLive.Import, :import

    # Folder-to-competitor association
    live "/events/:event_id/folders", FolderLive.Associate, :associate

    # Order management
    live "/events/:event_id/orders", OrderLive.Index, :index
    live "/events/:event_id/orders/:id", OrderLive.Show, :show

    # Product template management
    live "/products", ProductLive.Index, :index
    live "/products/new", ProductLive.Form, :new
    live "/products/:id/edit", ProductLive.Form, :edit

    get "/photos/thumbnail/:id", PhotoController, :thumbnail
    get "/photos/preview/:id", PhotoController, :preview
  end

  # Other scopes may use custom stacks.
  # scope "/api", PhotoFinishWeb do
  #   pipe_through :api
  # end

  # Enable LiveDashboard and Swoosh mailbox preview in development
  if Application.compile_env(:photo_finish, :dev_routes) do
    # If you want to use the LiveDashboard in production, you should put
    # it behind authentication and allow only admins to access it.
    # If your application does not have an admins-only section yet,
    # you can use Plug.BasicAuth to set up some basic authentication
    # as long as you are also using SSL (which you should anyway).
    import Phoenix.LiveDashboard.Router

    scope "/dev" do
      pipe_through :browser

      live_dashboard "/dashboard", metrics: PhotoFinishWeb.Telemetry
      forward "/mailbox", Plug.Swoosh.MailboxPreview
    end
  end

  if Application.compile_env(:photo_finish, :dev_routes) do
    import AshAdmin.Router

    scope "/ash_admin" do
      pipe_through :browser

      ash_admin "/"
    end
  end

  defp basic_auth(conn, _opts) do
    username = System.get_env("ADMIN_USERNAME") || "admin"
    password = System.get_env("ADMIN_PASSWORD") || "secret"
    Plug.BasicAuth.basic_auth(conn, username: username, password: password)
  end
end
