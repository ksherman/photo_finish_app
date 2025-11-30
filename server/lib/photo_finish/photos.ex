defmodule PhotoFinish.Photos do
  use Ash.Domain,
    otp_app: :photo_finish

  resources do
    resource PhotoFinish.Photos.Photo
  end
end
