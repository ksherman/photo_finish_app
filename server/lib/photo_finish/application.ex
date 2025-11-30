defmodule PhotoFinish.Application do
  # See https://hexdocs.pm/elixir/Application.html
  # for more information on OTP Applications
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      PhotoFinishWeb.Telemetry,
      PhotoFinish.Repo,
      {DNSCluster,
       query: Application.get_env(:photo_finish, :dns_cluster_query) || :ignore},
      {Oban,
       AshOban.config(
         Application.fetch_env!(:photo_finish, :ash_domains),
         Application.fetch_env!(:photo_finish, Oban)
       )},
      {Phoenix.PubSub, name: PhotoFinish.PubSub},
      # Start a worker by calling: PhotoFinish.Worker.start_link(arg)
      # {PhotoFinish.Worker, arg},
      # Start to serve requests, typically the last entry
      PhotoFinishWeb.Endpoint,
      {AshAuthentication.Supervisor, [otp_app: :photo_finish]}
    ]

    # See https://hexdocs.pm/elixir/Supervisor.html
    # for other strategies and supported options
    opts = [strategy: :one_for_one, name: PhotoFinish.Supervisor]
    Supervisor.start_link(children, opts)
  end

  # Tell Phoenix to update the endpoint configuration
  # whenever the application is updated.
  @impl true
  def config_change(changed, _new, removed) do
    PhotoFinishWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
