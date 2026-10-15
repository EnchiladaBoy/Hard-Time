// Exercise the production wrapper with SDL virtual devices, including removal
// while a caller still holds a pointer to the disconnected device.
#define SDL_MAIN_HANDLED
#include <bb/runtime.sdl/joystick.h>
#include <iostream>
#include <stdexcept>

bool bbRuntimeIdle(){ SDL_PumpEvents();return true; }
static void require(bool condition,const char *message){ if(!condition) throw std::runtime_error(message); }

int main(){
    SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS,"1");
    require(SDL_Init(SDL_INIT_JOYSTICK|SDL_INIT_EVENTS)==0,SDL_GetError());
    {
        SDLInputDriver driver;
        const int original=driver.numJoysticks();
        int index=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,2,4,0);
        require(index>=0,"Cannot attach virtual controller");
        SDL_Joystick *device=SDL_JoystickOpen(index);
        driver.refresh();
        require(driver.numJoysticks()==original+1,"New controller was not discovered");
        BBDevice *held=driver.getJoystick(original);
        SDL_JoystickSetVirtualAxis(device,0,20000);
        SDL_JoystickSetVirtualButton(device,0,1);
        SDL_JoystickUpdate();
        require(held->getAxisState(0)>0.5f && held->keyDown(1),"Controller input was not mapped");
        SDL_JoystickDetachVirtual(index);driver.refresh();
        require(driver.numJoysticks()==original,"Disconnected controller remains active");
        require(!held->keyDown(1) && held->getAxisState(0)==0,"Disconnect left stuck controls");
        SDL_JoystickClose(device);
        index=SDL_JoystickAttachVirtual(SDL_JOYSTICK_TYPE_GAMECONTROLLER,2,4,0);
        device=SDL_JoystickOpen(index);driver.refresh();
        require(driver.numJoysticks()==original+1,"Controller did not reconnect");
        SDL_JoystickSetVirtualButton(device,1,1);SDL_JoystickUpdate();
        require(driver.getJoystick(original)->keyDown(2),"Reconnected input is inactive");
        SDL_JoystickDetachVirtual(index);driver.refresh();SDL_JoystickClose(device);
    }
    SDL_Quit();
    std::cout<<"PASS virtual controller attach, input, disconnect, stale handle and reconnect\n";
}
