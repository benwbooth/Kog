/**
 * vscrollbar
 *
 * @author mpdeimos
 * @date 2009/05/01
 */

#include <lib/std.mi>
#include <lib/com/autorepeatbutton.m>

Global AutoRepeatButton up, down;
Global Slider s;

System.onScriptLoaded ()
{
	AutoRepeat_Load();
	AutoRepeat_SetInitalDelay(100);
	up = getScriptGroup().getObject("up");
	down = getScriptGroup().getObject("down");
	s = getScriptGroup().getObject("slider");
}

System.onScriptUnloading()
{
	AutoRepeat_Unload();
}

up.onLeftClick ()
{
	if (AutoRepeat_ClickType)
	{
		s.setPosition(s.getPosition()+5);
	}
}

down.onLeftClick ()
{
	if (AutoRepeat_ClickType)
	{
		s.setPosition(s.getPosition()-5);
	}
}