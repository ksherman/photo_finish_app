defmodule PhotoFinish.Accounts do
  use Ash.Domain, otp_app: :photo_finish, extensions: [AshAdmin.Domain]

  admin do
    show? true
  end

  resources do
    resource PhotoFinish.Accounts.Token
    resource PhotoFinish.Accounts.User
    resource PhotoFinish.Accounts.ApiKey
  end
end
