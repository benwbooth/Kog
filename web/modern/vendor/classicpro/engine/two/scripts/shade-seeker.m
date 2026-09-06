#include <lib/std.mi>

#include <lib/config.mi>
#include <lib/pldir.mi>
#include <lib/quickPlaylist.mi>

Function setSeekerPos();
Function lightToggle(boolean OnOff);

Global Group g, g_seekerActive, g_seekerFinder;
Global Container c;
Global GuiObject l_seekerActive, l_seekerFinder, l_light, l_seekerHover;
Global int i_light, i_width;
Global Slider s_seeker0, s_seeker1;
Global Timer t_light;
Global Boolean leftButton, rightButton;

System.onScriptLoaded() {
	g = getScriptGroup();
	s_seeker0 = g.getObject("shade.seeker.slider.0");
	s_seeker1 = g.getObject("shade.seeker.slider.1");

	g_seekerFinder = g.getObject("shade.seeker.finder");
	l_seekerFinder = g_seekerFinder.getObject("shade.seeker.finder.layer");

	g_seekerActive = g.getObject("shade.seeker.active");
	l_light = g_seekerActive.getObject("shade.seeker.active.light");
	l_seekerActive = g_seekerActive.getObject("shade.seeker.active.layer");
	l_seekerHover = g_seekerActive.getObject("shade.seeker.hover.layer");

	//g_seekerInactive = g.getObject("two.info.seeker.inactive");
	//l_seekerInactive = g_seekerInactive.getObject("two.info.seeker.inactive.layer");

	Map m = new Map;
	m.loadMap("info.bg.seeker.light");
	i_light = m.getWidth();
	delete m;
	
	t_light = new Timer;
	t_light.setDelay(5000);
	
	if(System.getStatus()==STATUS_PLAYING){
		lightToggle(true);
	}
}

System.onScriptUnloading(){
}

s_seeker0.onPostedPosition(int newpos){
	setSeekerPos();
}

s_seeker1.onSetPosition(int newpos){
	int temp = g.getWidth();
	
	if(leftButton && rightButton) g_seekerFinder.setXmlParam("w", "0");
	else g_seekerFinder.setXmlParam("w", integerToString(temp*newpos/255));
}
s_seeker1.onSetFinalPosition(int pos){
	g_seekerFinder.setXmlParam("w", "0");
}

s_seeker1.onLeftButtonDown(int x, int y){
	leftButton=true;
	if(rightButton) rightButton=false;
}
s_seeker1.onLeftButtonUp(int x, int y){
	leftButton=false;
}
s_seeker1.onRightButtonDown(int x, int y){
	rightButton=true;
}
s_seeker1.onRightButtonUp(int x, int y){
	rightButton=false;

	if(leftButton){
		g_seekerFinder.setXmlParam("w", "0");
		complete;
	}
	else{
		popQuickPlaylist(System.getViewportHeight()/24, false);
		complete;
	}
}


g.onResize(int x, int y, int w, int h){
	l_seekerActive.setXmlParam("w", integerToString(w));
	l_seekerFinder.setXmlParam("w", integerToString(w));
	l_seekerHover.setXmlParam("w", integerToString(w));
	i_width = w;
	
	//l_seekerInactive.setXmlParam("x", integerToString(-w));
	//l_seekerInactive.setXmlParam("w", integerToString(w));

	setSeekerPos();
}

setSeekerPos(){
	int temp = g.getWidth();
	int newpos = System.getPosition();
	int fullpos = System.getPlayItemLength();
	int fix = 1;
	
	if(System.getStatus()==STATUS_STOPPED || fullpos==0){
		newpos = 0;
		fullpos = 1;
		fix = 0;
	}
	

	//g_seekerInactive.setXmlParam("x", integerToString(temp*newpos/fullpos));
	//g_seekerInactive.setXmlParam("w", integerToString(temp-temp*newpos/fullpos+fix));

	//g_seekerInactive.setTargetX(temp*newpos/fullpos);
	//g_seekerInactive.setTargetW(temp-temp*newpos/fullpos+1);
	//g_seekerInactive.setTargetSpeed(0);
	//g_seekerInactive.gotoTarget();
	g_seekerActive.setXmlParam("w", integerToString(temp*newpos/fullpos));

}


s_seeker1.onEnterArea(){
	l_seekerHover.cancelTarget();
	l_seekerHover.setTargetA(255);
	l_seekerHover.setTargetW(i_width);
	l_seekerHover.setTargetSpeed(0.2);
	l_seekerHover.gotoTarget();

}
s_seeker1.onLeaveArea(){
	l_seekerHover.cancelTarget();
	l_seekerHover.setTargetA(0);
	l_seekerHover.setTargetW(i_width);
	l_seekerHover.setTargetSpeed(0.8);
	l_seekerHover.gotoTarget();
}

System.onStop(){
	setSeekerPos();
	lightToggle(false);
}
System.onPlay(){
	lightToggle(true);
}
System.onPause(){
	lightToggle(false);
}

System.onResume(){
	lightToggle(true);
}

t_light.onTimer(){
	l_light.cancelTarget();
	l_light.setXmlParam("x", integerToString(-i_light));
	l_light.show();
	l_light.setTargetX(g.getWidth());
	l_light.setTargetSpeed(1.5);
	l_light.gotoTarget();
	t_light.setDelay(5000);
}

l_light.onTargetReached(){
	l_light.hide();
}

lightToggle(boolean OnOff){
	if(OnOff){
		t_light.setDelay(2000);
		t_light.start();
	}
	else{
		t_light.stop();
		l_light.hide();
	}
}