defmodule PhotoFinishServer.Secrets do
  use AshAuthentication.Secret

  def secret_for(
        [:authentication, :tokens, :signing_secret],
        PhotoFinishServer.Accounts.User,
        _opts,
        _context
      ) do
    Application.fetch_env(:photo_finish_server, :token_signing_secret)
  end
end
