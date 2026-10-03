Rails.application.routes.draw do
  resources :widgets, only: %i[index show create update]
end
