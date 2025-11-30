defmodule PhotoFinishWeb.PageController do
  use PhotoFinishWeb, :controller

  def home(conn, _params) do
    render(conn, :home)
  end
end
