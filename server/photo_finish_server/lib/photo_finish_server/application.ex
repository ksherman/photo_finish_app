defmodule PhotoFinishServer.Application do
  # See https://hexdocs.pm/elixir/Application.html
  # for more information on OTP Applications
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      PhotoFinishServerWeb.Telemetry,
      PhotoFinishServer.Repo,
      {DNSCluster,
       query: Application.get_env(:photo_finish_server, :dns_cluster_query) || :ignore},
      {Oban,
       AshOban.config(
         Application.fetch_env!(:photo_finish_server, :ash_domains),
         Application.fetch_env!(:photo_finish_server, Oban)
       )},
      {Phoenix.PubSub, name: PhotoFinishServer.PubSub},
      # Start a worker by calling: PhotoFinishServer.Worker.start_link(arg)
      # {PhotoFinishServer.Worker, arg},
      # Start to serve requests, typically the last entry
      PhotoFinishServerWeb.Endpoint,
      {AshAuthentication.Supervisor, [otp_app: :photo_finish_server]}
    ]

    # See https://hexdocs.pm/elixir/Supervisor.html
    # for other strategies and supported options
    opts = [strategy: :one_for_one, name: PhotoFinishServer.Supervisor]
    Supervisor.start_link(children, opts)
  end

  # Tell Phoenix to update the endpoint configuration
  # whenever the application is updated.
  @impl true
  def config_change(changed, _new, removed) do
    PhotoFinishServerWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
