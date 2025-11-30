defmodule PhotoFinishServerWeb.PageController do
  use PhotoFinishServerWeb, :controller

  def home(conn, _params) do
    render(conn, :home)
  end
end
