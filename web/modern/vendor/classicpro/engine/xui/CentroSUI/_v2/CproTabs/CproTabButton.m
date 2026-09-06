/**
 * ClassicProTabButton.m
 *
 * Tab Button handling including moveable tabs
 *
 * @package	com.skinconsortium.cpro.one
 * @author	mpdeimos
 * @date	2008/11/21
 */

#include <lib/std.mi>

#include ../../../../scripts/dispatch_codes.m

Function setButtonState(int mode);

Global ToggleButton trigger;
Global Group parent;
Global GuiObject grid;
Global Text label;
Global boolean wasActive, movingTab, mouseDown, moved, dblClick;
Global int dx;
Global int yx = 0;
Global Layer icon;
Global boolean moreIcons;

// HACK (mpdeimos) this timer is a big HACK! Remove it if possible
// needed to recieve button deactivated msgs (mainly to send the action)
// test w/ GTAIV skin
Global timer tmr;

System.onScriptLoaded ()
{
	setDispatcher(getScriptGroup().getParent());

	parent = getScriptGroup();
	trigger = parent.getObject("cpro.tab.button");
	label = parent.getObject("cpro.tab.text");
	grid = parent.getObject("cpro.tab.grid");
	icon = parent.getObject("cpro.tab.icon");
	
	tmr = new Timer;
	tmr.setDelay(10);
	
	//debugint(icon.getLength());
	Map m = new Map;
	m.loadMap("cpro.tab.icon.ple.3");
	if(m.getHeight()==25) moreIcons = true;
	/*else{
		String temp = icon.getXmlParam("tooltip");
		debug(strleft(temp,strlen(temp)-1));
		//icon.setXmlParam("image", strleft(temp,strlen(temp)-1));
	}*/
	delete m;

	//moreIcons = true;
}

System.onScriptUnloading ()
{
	tmr.stop();
	delete tmr;
}

trigger.onLeftButtonDblClk(int x, int y){
	dblClick=true;
	//sendMessage(ON_LEFT_DBL_CLICK, 0, 0, 0, "", "", parent);
}


trigger.onLeftButtonDown (int x, int y)
{
	mouseDown=true;
	wasActive = getActivated();
	dx = x;

	sendMessage(ON_LEFT_BUTTON_DOWN, x, x, 0, "", "", parent);
}

trigger.onMouseMove (int x, int y)
{
	if(mouseDown){
		sendMessage(ON_MOUSE_MOVE, x, x, 0, "", "", parent);
		moved = true;
		if(!movingTab){
			if (dx > x+4 || dx < x-4){
				movingTab=true;
			}
		}
	}
}

trigger.onLeftButtonUp (int x, int y)
{
	mouseDown=false;
	if (!movingTab)
	{
		if(!wasActive) sendMessageO(ON_TAB_ACTIVATED, parent);
		setActivated(1);
	}
	else
	{
		setActivated(wasActive);
	}
	
	movingTab=false;
	sendMessage(ON_LEFT_BUTTON_UP, x, x, moved, "", "", parent);
	moved = false;

	if(dblClick){
		dblClick=false;
		sendMessage(ON_LEFT_DBL_CLICK, 0, 0, 0, "", "", parent);
	}
}

trigger.onRightButtonUp (int x, int y)
{
	if (movingTab)
	{
		complete;
		return;
	}
	
	sendMessage(ON_RIGHT_BUTTON_UP, x, clientToScreenX(trigger.getLeft()), clientToScreenY(trigger.getTop()+26), "", "", parent);
}

trigger.onActivate (boolean onOff)
{
	if (onOff)
	{
		setButtonState(1);
	}
	else
	{
		setButtonState(2);
		tmr.stop();
		tmr.start();
	}
}

tmr.onTimer ()
{
	tmr.stop();
	sendMessageO(ON_TAB_OFF, parent);
}


trigger.onEnterArea()
{
	if (!trigger.getActivated())
	{
		setButtonState(3);
		sendMessageO(ON_ENTER_AREA, parent);
	}
}

trigger.onLeaveArea()
{
	if (!trigger.getActivated())
	{
		setButtonState(2);
		sendMessageO(ON_LEAVE_AREA, parent);
	}
}

setButtonState (int mode)
{
	grid.setXmlParam("topleft", "sui.tab.left."+integerToString(mode));
	grid.setXmlParam("top", "sui.tab.center."+integerToString(mode));
	grid.setXmlParam("topright", "sui.tab.right."+integerToString(mode));
	label.setXmlParam("color", "");

	if (mode == 1)
	{
		icon.setXmlParam("y", integerToString(-1+yx));
		if(moreIcons) icon.setXmlParam("image", icon.getXmlParam("tooltip")+".3");
		//debug(icon.getXmlParam("tooltip"));
		label.setXmlParam("y", integerToString(-1+yx));
		label.setXmlParam("color", "cpro2.color.tab.on");
	}
	else if (mode == 2)
	{
		icon.setXmlParam("y", integerToString(0+yx));
		if(moreIcons) icon.setXmlParam("image", icon.getXmlParam("tooltip")+".1");
		label.setXmlParam("y", integerToString(0+yx));
		label.setXmlParam("color", "cpro2.color.tab.off");
	}
	else
	{
		icon.setXmlParam("y", integerToString(0+yx));
		if(moreIcons) icon.setXmlParam("image", icon.getXmlParam("tooltip")+".2");
		label.setXmlParam("y", integerToString(0+yx));
		label.setXmlParam("color", "cpro2.color.tab.hover");
	}
}

parent.onSetVisible(boolean onOff){
	if(onOff){
		String tabset = getPrivateString(getSkinName(), "tabtext", "arial");
		String tabtoken = "";
		String param1 = "";
		String param2 = "";
		for(int i = 0; i<20;i++){
			tabtoken = getToken(tabset,";",i);
			if(tabtoken=="") break;
			
			param1 = getToken(tabtoken,",",0);
			param2 = getToken(tabtoken,",",1);
			
			if(param1=="y") yx= stringToInteger(param2);
			else label.setXmlParam(param1, param2);		
		}
		
		trigger.onActivate(trigger.getActivated());
	}
}
/*
label.onTextChanged (String newtxt)
{
	trigger.setXmlParam("tooltip", newtxt);
	sendMessageO(ON_TAB_TEXT_UPDATED, parent);
}
*/