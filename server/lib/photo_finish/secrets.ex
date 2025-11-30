defmodule PhotoFinish.Secrets do
  use AshAuthentication.Secret

  def secret_for(
        [:authentication, :tokens, :signing_secret],
        PhotoFinish.Accounts.User,
        _opts,
        _context
      ) do
    Application.fetch_env(:photo_finish, :token_signing_secret)
  end
end
