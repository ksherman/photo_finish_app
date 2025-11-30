defmodule PhotoFinishServer.Accounts do
  use Ash.Domain, otp_app: :photo_finish_server, extensions: [AshAdmin.Domain]

  admin do
    show? true
  end

  resources do
    resource PhotoFinishServer.Accounts.Token
    resource PhotoFinishServer.Accounts.User
    resource PhotoFinishServer.Accounts.ApiKey
  end
end
